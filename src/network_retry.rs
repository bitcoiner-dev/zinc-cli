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
