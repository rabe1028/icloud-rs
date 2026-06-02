use icloud_protocol::{RequestSpec, ResponseParts};

pub trait AsyncTransport {
    type Error: std::error::Error + Send + Sync + 'static;

    fn execute(
        &self,
        request: RequestSpec,
    ) -> impl Future<Output = Result<ResponseParts, Self::Error>> + '_;
}

pub trait BlockingTransport {
    type Error: std::error::Error + Send + Sync + 'static;

    fn execute(&self, request: RequestSpec) -> Result<ResponseParts, Self::Error>;
}
