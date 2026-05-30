#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusCode {
    Ok,
    Created,
    MovedPermanently,
    BadRequest,
    Forbidden,
    NotFound,
    MethodNotAllowed,
    PayloadTooLarge,
    InternalServerError,
}

impl StatusCode {
    pub fn as_u16(self) -> u16 {
        match self {
            Self::Ok => 200,
            Self::Created => 201,
            Self::MovedPermanently => 301,
            Self::BadRequest => 400,
            Self::Forbidden => 403,
            Self::NotFound => 404,
            Self::MethodNotAllowed => 405,
            Self::PayloadTooLarge => 413,
            Self::InternalServerError => 500,
        }
    }

    pub fn reason_phrase(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Created => "Created",
            Self::MovedPermanently => "Moved Permanently",
            Self::BadRequest => "Bad Request",
            Self::Forbidden => "Forbidden",
            Self::NotFound => "Not Found",
            Self::MethodNotAllowed => "Method Not Allowed",
            Self::PayloadTooLarge => "Payload Too Large",
            Self::InternalServerError => "Internal Server Error",
        }
    }

    pub fn from_u16(code: u16) -> Option<Self> {
        match code {
            200 => Some(Self::Ok),
            201 => Some(Self::Created),
            301 => Some(Self::MovedPermanently),
            400 => Some(Self::BadRequest),
            403 => Some(Self::Forbidden),
            404 => Some(Self::NotFound),
            405 => Some(Self::MethodNotAllowed),
            413 => Some(Self::PayloadTooLarge),
            500 => Some(Self::InternalServerError),
            _ => None,
        }
    }
}
