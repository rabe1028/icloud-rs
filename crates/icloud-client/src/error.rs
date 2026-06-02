use thiserror::Error;

use icloud_types::ICloudError;

/// Error returned by client operations, parameterized by the transport's own
/// error type.
#[derive(Debug, Error)]
pub enum ClientError<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    #[error(transparent)]
    ICloud(#[from] ICloudError),
    #[error("transport error: {0}")]
    Transport(#[source] E),
}
