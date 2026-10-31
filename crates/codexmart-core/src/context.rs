//! Who is acting, when, and what they are allowed to do.

use chrono::{DateTime, Local, NaiveDate, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Manager,
    Cashier,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Manager => "manager",
            Role::Cashier => "cashier",
        }
    }

    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "admin" => Ok(Role::Admin),
            "manager" => Ok(Role::Manager),
            "cashier" => Ok(Role::Cashier),
            other => Err(AppError::validation(format!("Unknown role '{other}'."))),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Role::Admin => "Administrator",
            Role::Manager => "Manager",
            Role::Cashier => "Cashier",
        }
    }

    /// The single permission matrix. The UI only uses it to hide controls;
    /// every service re-checks it.
    pub fn allows(self, p: Permission) -> bool {
        use Permission::*;
        match self {
            Role::Admin => true,
            Role::Manager => !matches!(p, ManageUsers | ManageSettings | RestoreBackup),
            Role::Cashier => matches!(p, Sell | ViewSales | ViewCatalog | ManageCustomers | ReceiveCustomerPayment),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Permission {
    Sell,
    ViewSales,
    VoidSale,
    ProcessReturn,
    ViewCatalog,
    ManageCatalog,
    ManageInventory,
    ManagePurchasing,
    ManageExpenses,
    ManageCustomers,
    ReceiveCustomerPayment,
    ViewReports,
    ViewCosts,
    CloseCash,
    CreateBackup,
    RestoreBackup,
    ManageSettings,
    ManageUsers,
    ViewAudit,
}

impl Permission {
    pub const ALL: [Permission; 19] = [
        Permission::Sell,
        Permission::ViewSales,
        Permission::VoidSale,
        Permission::ProcessReturn,
        Permission::ViewCatalog,
        Permission::ManageCatalog,
        Permission::ManageInventory,
        Permission::ManagePurchasing,
        Permission::ManageExpenses,
        Permission::ManageCustomers,
        Permission::ReceiveCustomerPayment,
        Permission::ViewReports,
        Permission::ViewCosts,
        Permission::CloseCash,
        Permission::CreateBackup,
        Permission::RestoreBackup,
        Permission::ManageSettings,
        Permission::ManageUsers,
        Permission::ViewAudit,
    ];

    fn describe(self) -> &'static str {
        use Permission::*;
        match self {
            Sell => "make sales",
            ViewSales => "view sales",
            VoidSale => "void sales",
            ProcessReturn => "process returns",
            ViewCatalog => "view products",
            ManageCatalog => "manage products",
            ManageInventory => "adjust stock",
            ManagePurchasing => "manage suppliers and purchases",
            ManageExpenses => "manage expenses",
            ManageCustomers => "manage customers",
            ReceiveCustomerPayment => "receive customer payments",
            ViewReports => "view reports",
            ViewCosts => "view cost prices",
            CloseCash => "close the cash drawer",
            CreateBackup => "create backups",
            RestoreBackup => "restore backups",
            ManageSettings => "change settings",
            ManageUsers => "manage users",
            ViewAudit => "view the audit log",
        }
    }
}

/// The signed-in user performing an operation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Actor {
    pub user_id: String,
    pub username: String,
    pub full_name: String,
    pub role: Role,
}

impl Actor {
    /// Used for automatic background work such as scheduled backups.
    pub fn system() -> Self {
        Actor { user_id: String::new(), username: "system".into(), full_name: "System".into(), role: Role::Admin }
    }

    pub fn permissions(&self) -> Vec<Permission> {
        Permission::ALL.iter().copied().filter(|p| self.role.allows(*p)).collect()
    }
}

/// Request context passed to every service call. `now` and `today` are
/// captured once so that every row written by one operation agrees on them
/// (and so tests can pin them).
#[derive(Debug, Clone)]
pub struct Ctx {
    pub actor: Actor,
    pub now: DateTime<Utc>,
    pub today: NaiveDate,
}

impl Ctx {
    pub fn new(actor: Actor) -> Self {
        Ctx { actor, now: Utc::now(), today: Local::now().date_naive() }
    }

    pub fn at(actor: Actor, now: DateTime<Utc>, today: NaiveDate) -> Self {
        Ctx { actor, now, today }
    }

    pub fn system() -> Self {
        Ctx::new(Actor::system())
    }

    pub fn ts(&self) -> String {
        self.now.to_rfc3339_opts(SecondsFormat::Millis, true)
    }

    pub fn date(&self) -> String {
        self.today.format("%Y-%m-%d").to_string()
    }

    /// `None` for the system actor, so foreign keys to `users` stay valid.
    pub fn user_id(&self) -> Option<&str> {
        if self.actor.user_id.is_empty() {
            None
        } else {
            Some(self.actor.user_id.as_str())
        }
    }

    pub fn can(&self, p: Permission) -> bool {
        self.actor.role.allows(p)
    }

    pub fn require(&self, p: Permission) -> AppResult<()> {
        if self.can(p) {
            Ok(())
        } else {
            Err(AppError::Forbidden(format!(
                "Your role ({}) is not allowed to {}. Ask an administrator.",
                self.actor.role.label(),
                p.describe()
            )))
        }
    }
}

pub fn timestamp(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(SecondsFormat::Millis, true)
}
