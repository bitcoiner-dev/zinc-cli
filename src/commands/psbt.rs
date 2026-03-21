use crate::cli::{Cli, PolicyMode, PsbtAction, PsbtArgs};
use crate::error::AppError;
use crate::network_retry::with_network_retry;
use crate::utils::{maybe_write_text, parse_indices, resolve_psbt_source};
use crate::wallet_service::map_wallet_error;
use crate::{load_wallet_session, persist_wallet_session};
use serde_json::{json, Value};
use zinc_core::*;

pub async fn run(cli: &Cli, args: &PsbtArgs) -> Result<Value, AppError> {
    let psbt_stdin = match &args.action {
        PsbtAction::Create { .. } => false,
        PsbtAction::Analyze { psbt_stdin, .. } => *psbt_stdin,
        PsbtAction::Sign { psbt_stdin, .. } => *psbt_stdin,
        PsbtAction::Broadcast { psbt_stdin, .. } => *psbt_stdin,
    };
    if cli.password_stdin && psbt_stdin {
        return Err(AppError::Invalid(
            "--password-stdin cannot be combined with --psbt-stdin".to_string(),
        ));
    }

    match &args.action {
        PsbtAction::Create {
            to,
            amount_sats,
            fee_rate,
            out_file,
        } => {
            let mut session = load_wallet_session(cli)?;
            let request = CreatePsbtRequest::from_parts(to, *amount_sats, *fee_rate)
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let psbt_res: Result<String, ZincError> = session.wallet.create_psbt_base64(&request);
            let psbt = psbt_res.map_err(|e: ZincError| AppError::Internal(e.to_string()))?;

            if let Some(path) = out_file {
                maybe_write_text(Some(&path.display().to_string()), &psbt)?;
            }
            persist_wallet_session(&mut session)?;
            Ok(json!({"psbt": psbt}))
        }
        PsbtAction::Analyze {
            psbt,
            psbt_file,
            psbt_stdin,
        } => {
            let source = resolve_psbt_source(
                psbt.as_deref(),
                psbt_file.as_ref().map(|p| p.to_str().unwrap()),
                *psbt_stdin,
            )?;
            let session = load_wallet_session(cli)?;
            let (parsed, policy) = analyze_psbt_with_policy(&session.wallet, &source)?;
            Ok(json!({
                "analysis": parsed,
                "safe_to_send": policy.safe_to_send,
                "inscription_risk": policy.inscription_risk,
                "policy_reasons": policy.policy_reasons,
                "policy": {
                    "safe_to_send": policy.safe_to_send,
                    "inscription_risk": policy.inscription_risk,
                    "reasons": policy.policy_reasons
                }
            }))
        }
        PsbtAction::Sign {
            psbt,
            psbt_file,
            psbt_stdin,
            sign_inputs,
            sighash,
            finalize,
            out_file,
        } => {
            let source = resolve_psbt_source(
                psbt.as_deref(),
                psbt_file.as_ref().map(|p| p.to_str().unwrap()),
                *psbt_stdin,
            )?;
            let mut session = load_wallet_session(cli)?;
            let (analysis, policy) = analyze_psbt_with_policy(&session.wallet, &source)?;
            enforce_policy_mode(cli, &policy)?;
            let sign_input_indices = parse_indices(sign_inputs.as_deref())?;
            let options = SignOptions {
                sign_inputs: if sign_input_indices.is_empty() {
                    None
                } else {
                    Some(sign_input_indices)
                },
                sighash: *sighash,
                finalize: *finalize,
            };
            let signed_res: Result<String, String> =
                session.wallet.sign_psbt(&source, Some(options));
            let signed = signed_res.map_err(|e| AppError::Invalid(e.to_string()))?;
            if let Some(path) = out_file {
                maybe_write_text(Some(&path.display().to_string()), &signed)?;
            }
            persist_wallet_session(&mut session)?;
            Ok(json!({
                "psbt": signed,
                "safe_to_send": policy.safe_to_send,
                "inscription_risk": policy.inscription_risk,
                "policy_reasons": policy.policy_reasons,
                "analysis": analysis
            }))
        }
        PsbtAction::Broadcast {
            psbt,
            psbt_file,
            psbt_stdin,
        } => {
            let source = resolve_psbt_source(
                psbt.as_deref(),
                psbt_file.as_ref().map(|p| p.to_str().unwrap()),
                *psbt_stdin,
            )?;
            let mut session = load_wallet_session(cli)?;
            let (analysis, policy) = analyze_psbt_with_policy(&session.wallet, &source)?;
            enforce_policy_mode(cli, &policy)?;
            let esplora_url = session.profile.esplora_url.clone();
            let txid: String =
                with_network_retry(cli, "psbt broadcast", &mut session.wallet, |wallet| {
                    let url = esplora_url.clone();
                    let psbt_source = source.clone();
                    Box::pin(async move {
                        wallet
                            .broadcast(&psbt_source, &url)
                            .await
                            .map_err(map_wallet_error)
                    })
                })
                .await?;
            persist_wallet_session(&mut session)?;
            Ok(json!({
                "txid": txid,
                "safe_to_send": policy.safe_to_send,
                "inscription_risk": policy.inscription_risk,
                "policy_reasons": policy.policy_reasons,
                "analysis": analysis
            }))
        }
    }
}

struct PsbtPolicyDecision {
    safe_to_send: bool,
    inscription_risk: &'static str,
    policy_reasons: Vec<String>,
}

fn derive_psbt_policy(analysis: &Value) -> PsbtPolicyDecision {
    let warning_level = analysis
        .get("warning_level")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_ascii_lowercase();

    let inscriptions_burned = analysis
        .get("inscriptions_burned")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let warnings: Vec<String> = analysis
        .get("warnings")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();

    let mut policy_reasons = Vec::new();
    if inscriptions_burned {
        policy_reasons.push("analysis indicates inscriptions may be burned".to_string());
    }
    if warning_level == "high" || warning_level == "critical" {
        policy_reasons.push(format!("warning_level={warning_level}"));
    }
    for warning in &warnings {
        let lower = warning.to_ascii_lowercase();
        if lower.contains("burn") || lower.contains("inscription") || lower.contains("unsafe") {
            policy_reasons.push(warning.clone());
        }
    }
    policy_reasons.sort();
    policy_reasons.dedup();

    let inscription_risk = if inscriptions_burned || warning_level == "critical" {
        "high"
    } else if warning_level == "high" {
        "medium"
    } else if warning_level == "moderate"
        || warning_level == "warning"
        || !policy_reasons.is_empty()
    {
        "low"
    } else if warning_level == "none" || warning_level == "safe" {
        "none"
    } else {
        "unknown"
    };

    let safe_to_send = !inscriptions_burned
        && warning_level != "critical"
        && !warnings
            .iter()
            .any(|w| w.to_ascii_lowercase().contains("burn"));

    PsbtPolicyDecision {
        safe_to_send,
        inscription_risk,
        policy_reasons,
    }
}

fn analyze_psbt_with_policy(
    wallet: &ZincWallet,
    source: &str,
) -> Result<(Value, PsbtPolicyDecision), AppError> {
    let analysis_res: Result<String, String> = wallet.analyze_psbt(source);
    let analysis = analysis_res.map_err(|e| AppError::Invalid(e.to_string()))?;
    let parsed: Value = serde_json::from_str(&analysis)
        .map_err(|e| AppError::Invalid(format!("invalid analysis json: {e}")))?;
    let policy = derive_psbt_policy(&parsed);
    Ok((parsed, policy))
}

fn strict_mode_should_block(policy: &PsbtPolicyDecision) -> bool {
    if !policy.safe_to_send {
        return true;
    }
    matches!(policy.inscription_risk, "medium" | "high" | "unknown")
}

fn enforce_policy_mode(cli: &Cli, policy: &PsbtPolicyDecision) -> Result<(), AppError> {
    if matches!(cli.policy_mode, PolicyMode::Strict) && strict_mode_should_block(policy) {
        let reasons = if policy.policy_reasons.is_empty() {
            "no explicit policy reasons returned".to_string()
        } else {
            policy.policy_reasons.join("; ")
        };
        return Err(AppError::Policy(format!(
            "strict policy mode blocked operation (inscription_risk={}, reasons={})",
            policy.inscription_risk, reasons
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{derive_psbt_policy, strict_mode_should_block};
    use serde_json::json;

    #[test]
    fn test_derive_psbt_policy_blocks_burn_risk() {
        let analysis = json!({
            "warning_level": "critical",
            "inscriptions_burned": true,
            "warnings": ["Would burn inscription UTXO"]
        });
        let policy = derive_psbt_policy(&analysis);
        assert!(!policy.safe_to_send);
        assert_eq!(policy.inscription_risk, "high");
        assert!(!policy.policy_reasons.is_empty());
    }

    #[test]
    fn test_derive_psbt_policy_allows_clean_send() {
        let analysis = json!({
            "warning_level": "none",
            "inscriptions_burned": false,
            "warnings": []
        });
        let policy = derive_psbt_policy(&analysis);
        assert!(policy.safe_to_send);
        assert_eq!(policy.inscription_risk, "none");
        assert!(policy.policy_reasons.is_empty());
    }

    #[test]
    fn test_strict_mode_blocks_unknown_or_higher_risk() {
        let unknown = json!({
            "warning_level": "unknown",
            "inscriptions_burned": false,
            "warnings": []
        });
        let medium = json!({
            "warning_level": "high",
            "inscriptions_burned": false,
            "warnings": []
        });

        let policy_unknown = derive_psbt_policy(&unknown);
        let policy_medium = derive_psbt_policy(&medium);

        assert!(strict_mode_should_block(&policy_unknown));
        assert!(strict_mode_should_block(&policy_medium));
    }
}
