use std::io;

use handlebars::{RenderError, TemplateError};
use thiserror::Error;

use crate::conventional;

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error(transparent)]
    Git(#[from] git2::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Template(#[from] TemplateError),
    #[error(transparent)]
    Parser(#[from] conventional::ParseError),
    #[error(transparent)]
    Render(#[from] RenderError),
    #[error(transparent)]
    Url(#[from] url::ParseError),
    #[error("check error")]
    Check,
    #[error("canceled by user")]
    CancelledByUser,
    #[error(transparent)]
    Dialoguer(#[from] dialoguer::Error),
}
