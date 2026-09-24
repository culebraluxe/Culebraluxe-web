//! Accounting, served: the reads, the three commands, and the rules that decide whether either may run.
//!
//! THIN ON PURPOSE. Every method here authorizes, validates, calls the repository, and audits — in that order. The
//! projections are the DAO's queries and the arithmetic is the database's; what this layer owns is the part that must not
//! be skippable: WHO may do it, and WHETHER what they asked for makes sense.
//!
//! VALIDATION RUNS BEFORE THE REPOSITORY. A command that fails validation must not have touched the database, which is
//! why the checks are here rather than in the DAO's SQL: a constraint violation is a 500-shaped answer to what is really
//! a bad request.

use async_trait::async_trait;
use db::{AccountingDao, DbResult};
use domain::{
    AccountingDashboard, AccountingError, CategoryShare, CreateExpenseCommand,
    CreateReceivableCommand, Expense, MarkReceivablePaidCommand, MarkReceivablePaidOutcome,
    PnlRequest, PnlStatement, Receivable,
};
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

use crate::service_support::{audit_result, authorize, CoreServiceError};

/// The resource every Accounting operation is authorized against. It resolves through the same kind-based policy as the
/// rest of the portal — queries readable, commands requiring an authenticated principal — so Accounting inherits the
/// portal's boundary rather than inventing one.
const RESOURCE: &str = "accounting";

#[async_trait]
pub trait AccountingRepository: Send {
    async fn receivables(&mut self) -> DbResult<Vec<Receivable>>;
    async fn expenses(&mut self) -> DbResult<Vec<Expense>>;
    /// Every posted expense by category, for the Expenses screen's breakdown.
    async fn expense_categories(&mut self) -> DbResult<Vec<CategoryShare>>;
    async fn dashboard(&mut self) -> DbResult<AccountingDashboard>;
    async fn pnl(&mut self, request: &PnlRequest) -> DbResult<PnlStatement>;
    async fn create_expense(&mut self, command: &CreateExpenseCommand) -> DbResult<String>;
    async fn create_receivable(&mut self, command: &CreateReceivableCommand) -> DbResult<String>;
    async fn mark_receivable_paid(
        &mut self,
        command: &MarkReceivablePaidCommand,
    ) -> DbResult<Option<MarkReceivablePaidOutcome>>;
}

#[async_trait]
impl AccountingRepository for AccountingDao {
    async fn receivables(&mut self) -> DbResult<Vec<Receivable>> {
        AccountingDao::receivables(self).await
    }

    async fn expenses(&mut self) -> DbResult<Vec<Expense>> {
        AccountingDao::expenses(self).await
    }

    async fn expense_categories(&mut self) -> DbResult<Vec<CategoryShare>> {
        AccountingDao::expense_categories(self).await
    }

    async fn dashboard(&mut self) -> DbResult<AccountingDashboard> {
        AccountingDao::dashboard(self).await
    }

    async fn pnl(&mut self, request: &PnlRequest) -> DbResult<PnlStatement> {
        AccountingDao::pnl(self, request).await
    }

    async fn create_expense(&mut self, command: &CreateExpenseCommand) -> DbResult<String> {
        AccountingDao::create_expense(self, command).await
    }

    async fn create_receivable(&mut self, command: &CreateReceivableCommand) -> DbResult<String> {
        AccountingDao::create_receivable(self, command).await
    }

    async fn mark_receivable_paid(
        &mut self,
        command: &MarkReceivablePaidCommand,
    ) -> DbResult<Option<MarkReceivablePaidOutcome>> {
        AccountingDao::mark_receivable_paid(self, command).await
    }
}

/// A validation failure from the domain is a business error, not a database one.
///
/// The code travels unchanged, which is what makes `RECEIVABLE_CONFLICT` still read as a conflict at the HTTP edge, and
/// the message is the one written for a human.
impl From<AccountingError> for CoreServiceError {
    fn from(error: AccountingError) -> Self {
        match error {
            AccountingError::Invalid { code, message } => CoreServiceError::business(code, message),
        }
    }
}

pub struct AccountingService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: AccountingRepository> AccountingService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }
}

impl<R: AccountingRepository> AccountingService<R> {
    /// The dashboard's projections.
    pub async fn dashboard(
        &mut self,
        context: &ServiceContext,
    ) -> Result<AccountingDashboard, CoreServiceError> {
        const OP: &str = "accounting.dashboard";
        let decision = authorize(
            &self.runtime,
            RESOURCE,
            "accounting.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.dashboard().await.map_err(Into::into);
        audit_result(&self.runtime, RESOURCE, OP, context, decision, &result).await?;
        result
    }

    pub async fn receivables(
        &mut self,
        context: &ServiceContext,
    ) -> Result<Vec<Receivable>, CoreServiceError> {
        const OP: &str = "accounting.receivables";
        let decision = authorize(
            &self.runtime,
            RESOURCE,
            "accounting.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.receivables().await.map_err(Into::into);
        audit_result(&self.runtime, RESOURCE, OP, context, decision, &result).await?;
        result
    }

    pub async fn expenses(
        &mut self,
        context: &ServiceContext,
    ) -> Result<Vec<Expense>, CoreServiceError> {
        const OP: &str = "accounting.expenses";
        let decision = authorize(
            &self.runtime,
            RESOURCE,
            "accounting.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.expenses().await.map_err(Into::into);
        audit_result(&self.runtime, RESOURCE, OP, context, decision, &result).await?;
        result
    }

    /// The Expenses screen's breakdown: every posted expense by category, with each one's share.
    pub async fn expense_categories(
        &mut self,
        context: &ServiceContext,
    ) -> Result<Vec<CategoryShare>, CoreServiceError> {
        const OP: &str = "accounting.expenseCategories";
        let decision = authorize(
            &self.runtime,
            RESOURCE,
            "accounting.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .expense_categories()
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, RESOURCE, OP, context, decision, &result).await?;
        result
    }

    /// The P&L for the period the caller asked for.
    ///
    /// THE RANGE IS THE CALLER'S AND IT IS VALIDATED HERE. A screen that filters to a period gets that period or an
    /// error saying why it cannot — never a different period that looked close enough.
    pub async fn pnl(
        &mut self,
        request: &PnlRequest,
        context: &ServiceContext,
    ) -> Result<PnlStatement, CoreServiceError> {
        const OP: &str = "accounting.pnl";
        let decision = authorize(
            &self.runtime,
            RESOURCE,
            "accounting.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = async {
            request.validate()?;
            self.repository.pnl(request).await.map_err(Into::into)
        }
        .await;
        audit_result(&self.runtime, RESOURCE, OP, context, decision, &result).await?;
        result
    }


    /// Record an expense.
    pub async fn create_expense(
        &mut self,
        command: &CreateExpenseCommand,
        context: &ServiceContext,
    ) -> Result<String, CoreServiceError> {
        const OP: &str = "accounting.createExpense";
        let decision = authorize(
            &self.runtime,
            RESOURCE,
            "accounting.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            // Validate the caller's words, then store the normalised form of them: what is checked and what is written
            // must be the same value, so a trailing space cannot pass validation and land in the database.
            command.validate()?;
            self.repository
                .create_expense(&command.normalised())
                .await
                .map_err(Into::into)
        }
        .await;
        audit_result(&self.runtime, RESOURCE, OP, context, decision, &result).await?;
        result
    }

    /// Record a receivable.
    pub async fn create_receivable(
        &mut self,
        command: &CreateReceivableCommand,
        context: &ServiceContext,
    ) -> Result<String, CoreServiceError> {
        const OP: &str = "accounting.createReceivable";
        let decision = authorize(
            &self.runtime,
            RESOURCE,
            "accounting.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            command.validate()?;
            self.repository
                .create_receivable(&command.normalised())
                .await
                .map_err(Into::into)
        }
        .await;
        audit_result(&self.runtime, RESOURCE, OP, context, decision, &result).await?;
        result
    }

    /// Mark a receivable paid.
    ///
    /// THE TRANSITION IS THE DATABASE'S. The repository reports whether a row was actually transitioned; when it was not,
    /// the answer is a conflict — the receivable does not exist, or it is void and therefore not payable. This is the
    /// same conflict the live screen reported, so a screen that races another operator still gets the honest answer.
    pub async fn mark_receivable_paid(
        &mut self,
        command: &MarkReceivablePaidCommand,
        context: &ServiceContext,
    ) -> Result<MarkReceivablePaidOutcome, CoreServiceError> {
        const OP: &str = "accounting.markReceivablePaid";
        let decision = authorize(
            &self.runtime,
            RESOURCE,
            "accounting.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            command.validate()?;
            let outcome = self
                .repository
                .mark_receivable_paid(&command.normalised())
                .await?;
            outcome.ok_or_else(|| {
                CoreServiceError::business("RECEIVABLE_CONFLICT", "Receivable not found or voided.")
            })
        }
        .await;
        audit_result(&self.runtime, RESOURCE, OP, context, decision, &result).await?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::Money;
    use service::{
        CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort, ServiceActor,
        ServiceActorKind, ServicePrincipal,
    };
    use std::sync::{Arc, Mutex};

    /// What the fake repository saw. Kept behind a mutex alongside the repository itself so a test holds both the service
    /// and the record of what it asked for.
    #[derive(Default)]
    struct Recorder {
        pnl_requests: Vec<PnlRequest>,
        created_expenses: Vec<CreateExpenseCommand>,
        created_receivables: Vec<CreateReceivableCommand>,
        paid: Vec<MarkReceivablePaidCommand>,
        /// What the database would answer to a mark-paid: `None` is "no row was transitioned".
        paid_outcome: Option<MarkReceivablePaidOutcome>,
    }

    /// A repository that answers from memory and records what it was asked.
    ///
    /// ITS METHODS ARE SYNCHRONOUS, and the async trait implementation below only wraps them. That is deliberate: an
    /// implementation that held a `MutexGuard` across an `await` would not be `Send`, and the whole point of this fake is
    /// to be usable as the `R` in an async service.
    #[derive(Clone, Default)]
    struct FakeRepository(Arc<Mutex<Recorder>>);

    impl FakeRepository {
        fn record_pnl(&self, request: &PnlRequest) -> PnlStatement {
            self.0.lock().unwrap().pnl_requests.push(request.clone());
            PnlStatement {
                from: request.from.clone(),
                to: request.to.clone(),
                total_income: Money::from_database("12000.00"),
                net_income: Money::from_database("11874.50"),
                ..Default::default()
            }
        }

        fn record_expense(&self, command: &CreateExpenseCommand) -> String {
            self.0
                .lock()
                .unwrap()
                .created_expenses
                .push(command.clone());
            "expense-1".to_owned()
        }

        fn record_receivable(&self, command: &CreateReceivableCommand) -> String {
            self.0
                .lock()
                .unwrap()
                .created_receivables
                .push(command.clone());
            "receivable-1".to_owned()
        }

        fn record_paid(&self, command: &MarkReceivablePaidCommand) -> Option<MarkReceivablePaidOutcome> {
            let mut recorder = self.0.lock().unwrap();
            recorder.paid.push(command.clone());
            recorder.paid_outcome.clone()
        }
    }

    #[async_trait]
    impl AccountingRepository for FakeRepository {
        async fn receivables(&mut self) -> DbResult<Vec<Receivable>> {
            unreachable!("this test does not list receivables")
        }

        async fn expenses(&mut self) -> DbResult<Vec<Expense>> {
            unreachable!("this test does not list expenses")
        }

        async fn expense_categories(&mut self) -> DbResult<Vec<CategoryShare>> {
            unreachable!("this test does not read the breakdown")
        }

        async fn dashboard(&mut self) -> DbResult<AccountingDashboard> {
            unreachable!("this test does not read the dashboard")
        }

        async fn pnl(&mut self, request: &PnlRequest) -> DbResult<PnlStatement> {
            Ok(self.record_pnl(request))
        }

        async fn create_expense(&mut self, command: &CreateExpenseCommand) -> DbResult<String> {
            Ok(self.record_expense(command))
        }

        async fn create_receivable(
            &mut self,
            command: &CreateReceivableCommand,
        ) -> DbResult<String> {
            Ok(self.record_receivable(command))
        }

        async fn mark_receivable_paid(
            &mut self,
            command: &MarkReceivablePaidCommand,
        ) -> DbResult<Option<MarkReceivablePaidOutcome>> {
            Ok(self.record_paid(command))
        }
    }

    fn service() -> (AccountingService<FakeRepository>, FakeRepository) {
        let repository = FakeRepository::default();
        let infrastructure = ServiceInfrastructure::new(
            Arc::new(DefaultAuthorizationPort),
            Arc::new(CapturingAuditPort::default()),
            Arc::new(CapturingDomainEventPort::default()),
        );
        (
            AccountingService::new(repository.clone(), infrastructure),
            repository,
        )
    }

    fn actor() -> ServiceContext {
        ServiceContext {
            actor: ServiceActor {
                id: None,
                kind: ServiceActorKind::System,
            },
            correlation_id: "accounting-test".into(),
            causation_id: None,
            principal: Some(ServicePrincipal {
                app_user_id: "user-1".into(),
                level: "USER".into(),
                role_codes: vec![],
                account_type: "internal".into(),
                entitlement_codes: vec![],
            }),
        }
    }

    /// A caller the portal would not have let in: no principal at all.
    fn guest() -> ServiceContext {
        ServiceContext {
            principal: None,
            ..actor()
        }
    }

    #[tokio::test]
    async fn the_pnl_projection_keeps_the_range_the_caller_asked_for() {
        let (mut service, repository) = service();
        let request = PnlRequest {
            from: "2026-03-01".into(),
            to: "2026-03-31".into(),
        };

        let statement = service.pnl(&request, &actor()).await.unwrap();

        // The repository was asked for exactly that period, and the statement says so. A filter that is silently widened,
        // narrowed or dropped is the defect this proves against.
        let asked = repository.0.lock().unwrap().pnl_requests.clone();
        assert_eq!(asked.len(), 1);
        assert_eq!(asked[0].from, "2026-03-01");
        assert_eq!(asked[0].to, "2026-03-31");
        assert_eq!(statement.from, "2026-03-01");
        assert_eq!(statement.to, "2026-03-31");
        assert_eq!(statement.total_income.as_str(), "12000.00");
        assert_eq!(statement.net_income.as_str(), "11874.50");
    }

    #[tokio::test]
    async fn a_backwards_period_is_refused_before_the_repository() {
        let (mut service, repository) = service();
        let error = service
            .pnl(
                &PnlRequest {
                    from: "2026-04-01".into(),
                    to: "2026-03-31".into(),
                },
                &actor(),
            )
            .await
            .unwrap_err();

        assert_eq!(error.code(), "PNL_RANGE_INVALID");
        assert!(repository.0.lock().unwrap().pnl_requests.is_empty());
    }

    #[tokio::test]
    async fn a_guest_cannot_run_an_accounting_command() {
        let (mut service, repository) = service();
        let command = CreateExpenseCommand {
            vendor: "Sunrise Fuel".into(),
            category: "Office".into(),
            amount: "125.50".into(),
            expense_on: "2026-03-04".into(),
            ..Default::default()
        };

        let error = service
            .create_expense(&command, &guest())
            .await
            .unwrap_err();

        assert_eq!(error.code(), "FORBIDDEN");
        // The denial happened before the write, which is the order that matters: a rejected command must not be a command
        // that already ran.
        assert!(repository.0.lock().unwrap().created_expenses.is_empty());
    }

    #[tokio::test]
    async fn an_invalid_expense_never_reaches_the_repository() {
        let (mut service, repository) = service();
        let command = CreateExpenseCommand {
            vendor: "Sunrise Fuel".into(),
            category: "Not A Category".into(),
            amount: "125.50".into(),
            expense_on: "2026-03-04".into(),
            ..Default::default()
        };

        let error = service
            .create_expense(&command, &actor())
            .await
            .unwrap_err();

        assert_eq!(error.code(), "EXPENSE_CATEGORY_INVALID");
        assert!(repository.0.lock().unwrap().created_expenses.is_empty());
    }

}
