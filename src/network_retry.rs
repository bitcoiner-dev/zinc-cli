use crate::cli::Cli;
use crate::error::AppError;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use tokio::time::{sleep, timeout};

type RetryFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, AppError>> + 'a>>;

fn retry_backoff_ms(attempt: u32) -> u64 {
    let exp = attempt.saturating_sub(1).min(4);
    let base = 250_u64.saturating_mul(1_u64 << exp);
    base.min(4000)
}

pub async fn with_network_retry<C, T, Op>(
    cli: &Cli,
    operation: &str,
    ctx: &mut C,
    mut op: Op,
) -> Result<T, AppError>
where
    Op: for<'a> FnMut(&'a mut C) -> RetryFuture<'a, T>,
{
    let timeout_secs = cli.network_timeout_secs.max(1);
    let attempts = cli.network_retries.saturating_add(1);

    for attempt in 1..=attempts {
        match timeout(Duration::from_secs(timeout_secs), op(ctx)).await {
            Ok(Ok(value)) => return Ok(value),
            Ok(Err(err)) => {
                if err.tag() == "network" && attempt < attempts {
                    sleep(Duration::from_millis(retry_backoff_ms(attempt))).await;
                    continue;
                }
                return Err(err);
            }
            Err(_) => {
                if attempt < attempts {
                    sleep(Duration::from_millis(retry_backoff_ms(attempt))).await;
                    continue;
                }
                return Err(AppError::Network(format!(
                    "{operation} timed out after {timeout_secs}s (attempt {attempt}/{attempts})"
                )));
            }
        }
    }

    unreachable!("retry loop should always return")
}

#[cfg(test)]
mod tests {
    use super::{retry_backoff_ms, with_network_retry};
    use crate::cli::Cli;
    use crate::error::AppError;
    use clap::Parser;
    use std::time::Duration;

    fn cli(timeout_secs: u64, retries: u32) -> Cli {
        Cli::try_parse_from([
            "zinc-cli",
            "--network-timeout-secs",
            &timeout_secs.to_string(),
            "--network-retries",
            &retries.to_string(),
            "version",
        ])
        .expect("cli parse")
    }

    #[test]
    fn backoff_grows_exponentially_then_caps() {
        assert_eq!(retry_backoff_ms(1), 250);
        assert_eq!(retry_backoff_ms(2), 500);
        assert_eq!(retry_backoff_ms(3), 1000);
        assert_eq!(retry_backoff_ms(4), 2000);
        assert_eq!(retry_backoff_ms(5), 4000);
        assert_eq!(retry_backoff_ms(50), 4000);
    }

    #[tokio::test]
    async fn returns_value_on_first_success() {
        let cli = cli(5, 3);
        let mut ctx = 0u32;
        let result: Result<&str, AppError> =
            with_network_retry(&cli, "op", &mut ctx, |c| {
                *c += 1;
                Box::pin(async move { Ok("ok") })
            })
            .await;
        assert_eq!(result.unwrap(), "ok");
        assert_eq!(ctx, 1, "should only attempt once on success");
    }

    #[tokio::test]
    async fn retries_on_network_error_then_succeeds() {
        let cli = cli(5, 3);
        let mut ctx = 0u32;
        let result: Result<u32, AppError> =
            with_network_retry(&cli, "op", &mut ctx, |c| {
                *c += 1;
                let attempt = *c;
                Box::pin(async move {
                    if attempt < 2 {
                        Err(AppError::Network("flaky".into()))
                    } else {
                        Ok(attempt)
                    }
                })
            })
            .await;
        assert_eq!(result.unwrap(), 2);
        assert_eq!(ctx, 2, "should retry once then succeed");
    }

    #[tokio::test]
    async fn non_network_error_is_not_retried() {
        let cli = cli(5, 5);
        let mut ctx = 0u32;
        let result: Result<(), AppError> =
            with_network_retry(&cli, "op", &mut ctx, |c| {
                *c += 1;
                Box::pin(async move { Err(AppError::Invalid("bad".into())) })
            })
            .await;
        assert!(matches!(result, Err(AppError::Invalid(_))));
        assert_eq!(ctx, 1, "non-network errors should not retry");
    }

    #[tokio::test]
    async fn exhausts_network_retries_and_returns_last_error() {
        let cli = cli(5, 1); // 2 attempts total
        let mut ctx = 0u32;
        let result: Result<(), AppError> =
            with_network_retry(&cli, "op", &mut ctx, |c| {
                *c += 1;
                Box::pin(async move { Err(AppError::Network("down".into())) })
            })
            .await;
        assert!(matches!(result, Err(AppError::Network(_))));
        assert_eq!(ctx, 2, "should attempt retries + 1 times");
    }

    #[tokio::test]
    async fn timeout_maps_to_network_error() {
        let cli = cli(1, 0); // single 1s attempt
        let mut ctx = 0u32;
        let result: Result<(), AppError> =
            with_network_retry(&cli, "slow-op", &mut ctx, |c| {
                *c += 1;
                Box::pin(async move {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    Ok(())
                })
            })
            .await;
        match result {
            Err(AppError::Network(msg)) => {
                assert!(msg.contains("slow-op"), "message should name the op: {msg}");
                assert!(msg.contains("timed out"), "message should mention timeout: {msg}");
            }
            other => panic!("expected network timeout error, got {other:?}"),
        }
        assert_eq!(ctx, 1);
    }
}
