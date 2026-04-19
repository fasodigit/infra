// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// Hand-written prost/tonic stubs for faso.canary.v1.
// This file is replaced by the tonic_build output once `cargo build` regenerates it.
// Keep in sync with proto/canary/v1/canary.proto.

// ---------------------------------------------------------------------------
// SloConfig
// ---------------------------------------------------------------------------

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SloConfig {
    #[prost(double, tag = "1")]
    pub error_rate_max: f64,
    #[prost(double, tag = "2")]
    pub latency_p99_max_ms: f64,
    #[prost(string, tag = "3")]
    pub prometheus_endpoint: ::prost::alloc::string::String,
}

// ---------------------------------------------------------------------------
// StartCanary
// ---------------------------------------------------------------------------

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StartCanaryRequest {
    #[prost(string, tag = "1")]
    pub service: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub image_tag: ::prost::alloc::string::String,
    #[prost(uint32, repeated, tag = "3")]
    pub stages: ::prost::alloc::vec::Vec<u32>,
    #[prost(uint64, tag = "4")]
    pub min_stage_duration_secs: u64,
    #[prost(message, optional, tag = "5")]
    pub slo: ::core::option::Option<SloConfig>,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StartCanaryResponse {
    #[prost(string, tag = "1")]
    pub canary_id: ::prost::alloc::string::String,
}

// ---------------------------------------------------------------------------
// PauseCanary
// ---------------------------------------------------------------------------

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PauseCanaryRequest {
    #[prost(string, tag = "1")]
    pub canary_id: ::prost::alloc::string::String,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PauseCanaryResponse {
    #[prost(message, optional, tag = "1")]
    pub status: ::core::option::Option<CanaryStatus>,
}

// ---------------------------------------------------------------------------
// AbortCanary
// ---------------------------------------------------------------------------

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AbortCanaryRequest {
    #[prost(string, tag = "1")]
    pub canary_id: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub reason: ::prost::alloc::string::String,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AbortCanaryResponse {
    #[prost(message, optional, tag = "1")]
    pub status: ::core::option::Option<CanaryStatus>,
}

// ---------------------------------------------------------------------------
// PromoteCanary
// ---------------------------------------------------------------------------

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PromoteCanaryRequest {
    #[prost(string, tag = "1")]
    pub canary_id: ::prost::alloc::string::String,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PromoteCanaryResponse {
    #[prost(message, optional, tag = "1")]
    pub status: ::core::option::Option<CanaryStatus>,
}

// ---------------------------------------------------------------------------
// GetCanaryStatus / ListCanaries
// ---------------------------------------------------------------------------

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetCanaryStatusRequest {
    #[prost(string, tag = "1")]
    pub canary_id: ::prost::alloc::string::String,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListCanariesRequest {
    #[prost(string, tag = "1")]
    pub service: ::prost::alloc::string::String,
    #[prost(int32, optional, tag = "2")]
    pub stage_filter: ::core::option::Option<i32>,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListCanariesResponse {
    #[prost(message, repeated, tag = "1")]
    pub canaries: ::prost::alloc::vec::Vec<CanaryStatus>,
}

// ---------------------------------------------------------------------------
// CanaryStatus / SloCompliance
// ---------------------------------------------------------------------------

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SloCompliance {
    #[prost(double, tag = "1")]
    pub observed_error_rate: f64,
    #[prost(double, tag = "2")]
    pub observed_latency_p99_ms: f64,
    #[prost(bool, tag = "3")]
    pub within_budget: bool,
    #[prost(message, optional, tag = "4")]
    pub measured_at: ::core::option::Option<::prost_types::Timestamp>,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CanaryStatus {
    #[prost(string, tag = "1")]
    pub canary_id: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub service: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub image_tag: ::prost::alloc::string::String,
    #[prost(enumeration = "Stage", tag = "4")]
    pub current_stage: i32,
    #[prost(uint32, tag = "5")]
    pub current_weight_pct: u32,
    #[prost(message, optional, tag = "6")]
    pub started_at: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(message, optional, tag = "7")]
    pub stage_started_at: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(message, optional, tag = "8")]
    pub slo_compliance: ::core::option::Option<SloCompliance>,
    #[prost(string, tag = "9")]
    pub rollback_reason: ::prost::alloc::string::String,
}

// ---------------------------------------------------------------------------
// Stage enum
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum Stage {
    Unspecified = 0,
    Stage1Pct = 1,
    Stage10Pct = 2,
    Stage50Pct = 3,
    Promoted = 4,
    RolledBack = 5,
    Paused = 6,
}

impl Stage {
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Stage::Unspecified => "STAGE_UNSPECIFIED",
            Stage::Stage1Pct => "STAGE_1_PCT",
            Stage::Stage10Pct => "STAGE_10_PCT",
            Stage::Stage50Pct => "STAGE_50_PCT",
            Stage::Promoted => "STAGE_PROMOTED",
            Stage::RolledBack => "STAGE_ROLLED_BACK",
            Stage::Paused => "STAGE_PAUSED",
        }
    }
}

// ---------------------------------------------------------------------------
// Tonic server trait and generated server wrapper
// ---------------------------------------------------------------------------

/// Generated server implementations.
pub mod canary_service_server {
    #![allow(
        unused_variables,
        dead_code,
        missing_docs,
        clippy::wildcard_imports,
        clippy::let_unit_value,
    )]
    use tonic::codegen::*;

    /// Trait that must be implemented by the gRPC service handler.
    #[async_trait]
    pub trait CanaryService: Send + Sync + 'static {
        async fn start_canary(
            &self,
            request: tonic::Request<super::StartCanaryRequest>,
        ) -> std::result::Result<tonic::Response<super::StartCanaryResponse>, tonic::Status>;

        async fn pause_canary(
            &self,
            request: tonic::Request<super::PauseCanaryRequest>,
        ) -> std::result::Result<tonic::Response<super::PauseCanaryResponse>, tonic::Status>;

        async fn abort_canary(
            &self,
            request: tonic::Request<super::AbortCanaryRequest>,
        ) -> std::result::Result<tonic::Response<super::AbortCanaryResponse>, tonic::Status>;

        async fn promote_canary(
            &self,
            request: tonic::Request<super::PromoteCanaryRequest>,
        ) -> std::result::Result<tonic::Response<super::PromoteCanaryResponse>, tonic::Status>;

        async fn get_canary_status(
            &self,
            request: tonic::Request<super::GetCanaryStatusRequest>,
        ) -> std::result::Result<tonic::Response<super::CanaryStatus>, tonic::Status>;

        async fn list_canaries(
            &self,
            request: tonic::Request<super::ListCanariesRequest>,
        ) -> std::result::Result<tonic::Response<super::ListCanariesResponse>, tonic::Status>;
    }

    #[derive(Debug)]
    pub struct CanaryServiceServer<T> {
        inner: Arc<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }

    impl<T> CanaryServiceServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }

        pub fn from_arc(inner: Arc<T>) -> Self {
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
                max_decoding_message_size: None,
                max_encoding_message_size: None,
            }
        }

        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> InterceptedService<Self, F>
        where
            F: tonic::service::Interceptor,
        {
            InterceptedService::new(Self::new(inner), interceptor)
        }

        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.accept_compression_encodings.enable(encoding);
            self
        }

        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.send_compression_encodings.enable(encoding);
            self
        }

        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.max_decoding_message_size = Some(limit);
            self
        }

        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.max_encoding_message_size = Some(limit);
            self
        }
    }

    impl<T, B> tonic::codegen::Service<http::Request<B>> for CanaryServiceServer<T>
    where
        T: CanaryService,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;

        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<std::result::Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/faso.canary.v1.CanaryService/StartCanary" => {
                    #[allow(non_camel_case_types)]
                    struct StartCanarySvc<T: CanaryService>(pub Arc<T>);
                    impl<T: CanaryService>
                        tonic::server::UnaryService<super::StartCanaryRequest>
                        for StartCanarySvc<T>
                    {
                        type Response = super::StartCanaryResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::StartCanaryRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut =
                                async move { <T as CanaryService>::start_canary(&inner, request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = Arc::clone(&inner);
                    let fut = async move {
                        let method = StartCanarySvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/faso.canary.v1.CanaryService/PauseCanary" => {
                    #[allow(non_camel_case_types)]
                    struct PauseCanarySvc<T: CanaryService>(pub Arc<T>);
                    impl<T: CanaryService>
                        tonic::server::UnaryService<super::PauseCanaryRequest>
                        for PauseCanarySvc<T>
                    {
                        type Response = super::PauseCanaryResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::PauseCanaryRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as CanaryService>::pause_canary(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = Arc::clone(&inner);
                    let fut = async move {
                        let method = PauseCanarySvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/faso.canary.v1.CanaryService/AbortCanary" => {
                    #[allow(non_camel_case_types)]
                    struct AbortCanarySvc<T: CanaryService>(pub Arc<T>);
                    impl<T: CanaryService>
                        tonic::server::UnaryService<super::AbortCanaryRequest>
                        for AbortCanarySvc<T>
                    {
                        type Response = super::AbortCanaryResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::AbortCanaryRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as CanaryService>::abort_canary(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = Arc::clone(&inner);
                    let fut = async move {
                        let method = AbortCanarySvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/faso.canary.v1.CanaryService/PromoteCanary" => {
                    #[allow(non_camel_case_types)]
                    struct PromoteCanarySvc<T: CanaryService>(pub Arc<T>);
                    impl<T: CanaryService>
                        tonic::server::UnaryService<super::PromoteCanaryRequest>
                        for PromoteCanarySvc<T>
                    {
                        type Response = super::PromoteCanaryResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::PromoteCanaryRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as CanaryService>::promote_canary(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = Arc::clone(&inner);
                    let fut = async move {
                        let method = PromoteCanarySvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/faso.canary.v1.CanaryService/GetCanaryStatus" => {
                    #[allow(non_camel_case_types)]
                    struct GetCanaryStatusSvc<T: CanaryService>(pub Arc<T>);
                    impl<T: CanaryService>
                        tonic::server::UnaryService<super::GetCanaryStatusRequest>
                        for GetCanaryStatusSvc<T>
                    {
                        type Response = super::CanaryStatus;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetCanaryStatusRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as CanaryService>::get_canary_status(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = Arc::clone(&inner);
                    let fut = async move {
                        let method = GetCanaryStatusSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/faso.canary.v1.CanaryService/ListCanaries" => {
                    #[allow(non_camel_case_types)]
                    struct ListCanariesSvc<T: CanaryService>(pub Arc<T>);
                    impl<T: CanaryService>
                        tonic::server::UnaryService<super::ListCanariesRequest>
                        for ListCanariesSvc<T>
                    {
                        type Response = super::ListCanariesResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ListCanariesRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as CanaryService>::list_canaries(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = Arc::clone(&inner);
                    let fut = async move {
                        let method = ListCanariesSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => Box::pin(async move {
                    Ok(http::Response::builder()
                        .status(200)
                        .header("grpc-status", tonic::Code::Unimplemented as i32 as i64)
                        .header("content-type", "application/grpc")
                        .body(empty_body())
                        .unwrap())
                }),
            }
        }
    }

    impl<T: CanaryService> Clone for CanaryServiceServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
                max_decoding_message_size: self.max_decoding_message_size,
                max_encoding_message_size: self.max_encoding_message_size,
            }
        }
    }

    impl<T: CanaryService> tonic::server::NamedService for CanaryServiceServer<T> {
        const NAME: &'static str = "faso.canary.v1.CanaryService";
    }
}

// ---------------------------------------------------------------------------
// Tonic client
// ---------------------------------------------------------------------------

/// Generated client implementations.
pub mod canary_service_client {
    #![allow(
        unused_variables,
        dead_code,
        missing_docs,
        clippy::wildcard_imports,
        clippy::let_unit_value,
    )]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;

    #[derive(Debug, Clone)]
    pub struct CanaryServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }

    impl CanaryServiceClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }

    impl<T> CanaryServiceClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }

        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }

        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> CanaryServiceClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<http::Request<tonic::body::BoxBody>>>::Error:
                Into<StdError> + Send + Sync,
        {
            CanaryServiceClient::new(InterceptedService::new(inner, interceptor))
        }

        pub async fn start_canary(
            &mut self,
            request: impl tonic::IntoRequest<super::StartCanaryRequest>,
        ) -> std::result::Result<tonic::Response<super::StartCanaryResponse>, tonic::Status>
        {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/faso.canary.v1.CanaryService/StartCanary",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(tonic::GrpcMethod::new("faso.canary.v1.CanaryService", "StartCanary"));
            self.inner.unary(req, path, codec).await
        }

        pub async fn pause_canary(
            &mut self,
            request: impl tonic::IntoRequest<super::PauseCanaryRequest>,
        ) -> std::result::Result<tonic::Response<super::PauseCanaryResponse>, tonic::Status>
        {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(tonic::Code::Unknown, format!("{}", e.into()))
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/faso.canary.v1.CanaryService/PauseCanary",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(tonic::GrpcMethod::new("faso.canary.v1.CanaryService", "PauseCanary"));
            self.inner.unary(req, path, codec).await
        }

        pub async fn abort_canary(
            &mut self,
            request: impl tonic::IntoRequest<super::AbortCanaryRequest>,
        ) -> std::result::Result<tonic::Response<super::AbortCanaryResponse>, tonic::Status>
        {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(tonic::Code::Unknown, format!("{}", e.into()))
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/faso.canary.v1.CanaryService/AbortCanary",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(tonic::GrpcMethod::new("faso.canary.v1.CanaryService", "AbortCanary"));
            self.inner.unary(req, path, codec).await
        }

        pub async fn promote_canary(
            &mut self,
            request: impl tonic::IntoRequest<super::PromoteCanaryRequest>,
        ) -> std::result::Result<tonic::Response<super::PromoteCanaryResponse>, tonic::Status>
        {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(tonic::Code::Unknown, format!("{}", e.into()))
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/faso.canary.v1.CanaryService/PromoteCanary",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(tonic::GrpcMethod::new("faso.canary.v1.CanaryService", "PromoteCanary"));
            self.inner.unary(req, path, codec).await
        }

        pub async fn get_canary_status(
            &mut self,
            request: impl tonic::IntoRequest<super::GetCanaryStatusRequest>,
        ) -> std::result::Result<tonic::Response<super::CanaryStatus>, tonic::Status>
        {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(tonic::Code::Unknown, format!("{}", e.into()))
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/faso.canary.v1.CanaryService/GetCanaryStatus",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(tonic::GrpcMethod::new(
                "faso.canary.v1.CanaryService",
                "GetCanaryStatus",
            ));
            self.inner.unary(req, path, codec).await
        }

        pub async fn list_canaries(
            &mut self,
            request: impl tonic::IntoRequest<super::ListCanariesRequest>,
        ) -> std::result::Result<tonic::Response<super::ListCanariesResponse>, tonic::Status>
        {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(tonic::Code::Unknown, format!("{}", e.into()))
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/faso.canary.v1.CanaryService/ListCanaries",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(tonic::GrpcMethod::new("faso.canary.v1.CanaryService", "ListCanaries"));
            self.inner.unary(req, path, codec).await
        }
    }
}
