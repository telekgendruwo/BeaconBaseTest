use thiserror::Error;
use serde::Serialize;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

#[derive(Error, Debug)]
pub enum BeaconError {
    #[error("{0}")]
    ScanError(String),

    #[error("{0}")]
    InferenceError(String),

    #[error("{0}")]
    ValidationError(String),

    #[error("payment required")]
    PaymentRequired {
        run_id: String,
        amount: String,
        base_addr: String,
        sol_addr: String,
    },

    #[error("{status} - {message}")]
    CloudError {
        status: u16,
        message: String,
    },

    #[error("{0}")]
    ParseError(String),

    #[error(transparent)]
    NetworkError(#[from] reqwest::Error),

    #[error("{0}")]
    DatabaseError(String),

    #[error("transaction hash already used")]
    TransactionAlreadyUsed,

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error("{0}")]
    Unknown(String),
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
}

impl IntoResponse for BeaconError {
    fn into_response(self) -> Response {
        match self {
            BeaconError::PaymentRequired { run_id, amount, base_addr, sol_addr } => {
                let body = Json(ErrorResponse {
                    success: false,
                    error: "Payment required".to_string(),
                });
                (
                    StatusCode::PAYMENT_REQUIRED,
                    [
                        ("x-payment-run-id", run_id),
                        ("x-payment-amount", amount),
                        ("x-payment-currency", "USDC".to_string()),
                        ("x-payment-address-base", base_addr),
                        ("x-payment-address-solana", sol_addr),
                    ],
                    body,
                ).into_response()
            }
            BeaconError::TransactionAlreadyUsed => {
                let body = Json(ErrorResponse {
                    success: false,
                    error: self.to_string(),
                });
                (StatusCode::CONFLICT, body).into_response()
            }
            BeaconError::CloudError { status, message } => {
                let body = Json(ErrorResponse {
                    success: false,
                    error: message,
                });
                (StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), body).into_response()
            }
            _ => {
                let body = Json(ErrorResponse {
                    success: false,
                    error: self.to_string(),
                });
                (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
            }
        }
    }
}
