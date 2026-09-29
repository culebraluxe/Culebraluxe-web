use domain::{
    CreateDealRequest, CreateDealResult, DealContractPortfolioItem, DealOwnerCandidate,
    DealPortfolioItem, DealPortfolioSnapshot, DealWorkspaceActivity, DealWorkspaceClient,
    DealWorkspaceCommand, DealWorkspaceCommandResult, DealWorkspaceDeal, DealWorkspaceOffer,
    DealWorkspaceParticipant, DealWorkspaceProperty, DealWorkspaceShowing, DealWorkspaceSnapshot,
    DealWorkspaceTask, DealableProperty,
};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{Database, DbFailure, DbResult};
mod deal_row;
mod command;
mod workspace;
mod deals;
#[allow(unused_imports)]
pub use deal_row::*;
#[allow(unused_imports)]
pub use command::*;
#[allow(unused_imports)]
pub use workspace::*;
#[allow(unused_imports)]
pub use deals::*;

