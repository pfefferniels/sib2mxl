#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Sib(#[from] libsib::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
    #[error("the score has no {0}")]
    Missing(&'static str),
}

pub type Result<T> = std::result::Result<T, Error>;
