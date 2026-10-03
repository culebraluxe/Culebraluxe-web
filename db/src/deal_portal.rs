use model::{
    CreateDealRequest, CreateDealResult, DealContractPortfolioItem, DealOwnerCandidate,
    DealPortfolioItem, DealPortfolioSnapshot, DealWorkspaceActivity, DealWorkspaceClient,
    DealWorkspaceCommand, DealWorkspaceCommandResult, DealWorkspaceDeal, DealWorkspaceOffer,
    DealWorkspaceParticipant, DealWorkspaceProperty, DealWorkspaceShowing, DealWorkspaceSnapshot,
    DealWorkspaceTask, DealableProperty,
};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{Database, DbFailure, DbResult};
mod command;
mod deal_row;
mod deals;
mod workspace;
#[allow(unused_imports)]
pub use command::*;
#[allow(unused_imports)]
pub use deal_row::*;
#[allow(unused_imports)]
pub use deals::*;
#[allow(unused_imports)]
pub use workspace::*;
