use fittings::{FittingsError, MethodRouter};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct DoubleParams {
    value: i32,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct DoubleResult {
    doubled: i32,
}

#[fittings::service]
trait CalculatorService {
    #[fittings::method(name = "math/double")]
    async fn double(&self, params: DoubleParams) -> Result<DoubleResult, FittingsError>;
}

struct Calculator;

impl CalculatorService for Calculator {
    async fn double(&self, params: DoubleParams) -> Result<DoubleResult, FittingsError> {
        Ok(DoubleResult {
            doubled: params.value * 2,
        })
    }
}

#[tokio::test]
async fn generated_router_dispatches_known_method() {
    let router = into_calculator_service_router(Calculator);

    let result = router
        .route(
            "math/double",
            fittings::serde_json::json!({"value": 21}),
            fittings::Metadata::default(),
        )
        .await
        .expect("known method should succeed");

    assert_eq!(result, fittings::serde_json::json!({"doubled": 42}));
}

#[tokio::test]
async fn generated_router_rejects_unknown_method() {
    let router = into_calculator_service_router(Calculator);

    let error = router
        .route(
            "double",
            fittings::serde_json::json!({}),
            fittings::Metadata::default(),
        )
        .await
        .expect_err("unknown method should fail");

    assert!(matches!(
        error,
        FittingsError::MethodNotFound(message) if message == "double"
    ));
}

#[tokio::test]
async fn generated_router_maps_decode_errors_to_invalid_params() {
    let router = into_calculator_service_router(Calculator);

    let error = router
        .route(
            "math/double",
            fittings::serde_json::json!({"value": "oops"}),
            fittings::Metadata::default(),
        )
        .await
        .expect_err("invalid params should fail");

    assert!(matches!(error, FittingsError::InvalidParams(_)));
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct UnitParams;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct AckResult {
    ok: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct UnserializableResult;

impl Serialize for UnserializableResult {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Err(serde::ser::Error::custom("cannot serialize result"))
    }
}

#[fittings::service]
trait FailureService {
    async fn fail(&self, params: UnitParams) -> Result<AckResult, FittingsError>;
    async fn nan(&self, params: UnitParams) -> Result<UnserializableResult, FittingsError>;
}

struct FailureExample;

impl FailureService for FailureExample {
    async fn fail(&self, _params: UnitParams) -> Result<AckResult, FittingsError> {
        Err(FittingsError::internal("service failed"))
    }

    async fn nan(&self, _params: UnitParams) -> Result<UnserializableResult, FittingsError> {
        Ok(UnserializableResult)
    }
}

#[tokio::test]
async fn generated_router_propagates_service_errors() {
    let router = into_failure_service_router(FailureExample);

    let error = router
        .route(
            "fail",
            fittings::serde_json::json!(null),
            fittings::Metadata::default(),
        )
        .await
        .expect_err("service error should be propagated");

    assert!(matches!(
        error,
        FittingsError::Internal(message) if message == "service failed"
    ));
}

#[tokio::test]
async fn generated_router_maps_result_encoding_errors_to_internal_error() {
    let router = into_failure_service_router(FailureExample);

    let error = router
        .route(
            "nan",
            fittings::serde_json::json!(null),
            fittings::Metadata::default(),
        )
        .await
        .expect_err("result encoding should fail");

    assert!(matches!(error, FittingsError::Internal(_)));
}
