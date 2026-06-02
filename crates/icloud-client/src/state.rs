use icloud_types::{AuthState, AuthenticatedSession, EstablishedSession, PendingTwoFactorAuth};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoggedOut;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingTwoFactor {
    pub pending: PendingTwoFactorAuth,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedPreBootstrap {
    pub session: EstablishedSession,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authenticated {
    pub session: AuthenticatedSession,
}

#[derive(Debug, Clone)]
pub enum RestoredAsyncClient<T> {
    LoggedOut(crate::AsyncICloudClient<T, LoggedOut>),
    PendingTwoFactor(crate::AsyncICloudClient<T, PendingTwoFactor>),
    AuthenticatedPreBootstrap(crate::AsyncICloudClient<T, AuthenticatedPreBootstrap>),
    Authenticated(crate::AsyncICloudClient<T, Authenticated>),
}

#[derive(Debug, Clone)]
pub enum RestoredBlockingClient<T> {
    LoggedOut(crate::BlockingICloudClient<T, LoggedOut>),
    PendingTwoFactor(crate::BlockingICloudClient<T, PendingTwoFactor>),
    AuthenticatedPreBootstrap(crate::BlockingICloudClient<T, AuthenticatedPreBootstrap>),
    Authenticated(crate::BlockingICloudClient<T, Authenticated>),
}

pub trait StateSnapshot {
    fn snapshot(&self) -> AuthState;
}

impl StateSnapshot for LoggedOut {
    fn snapshot(&self) -> AuthState {
        AuthState::LoggedOut
    }
}

impl StateSnapshot for PendingTwoFactor {
    fn snapshot(&self) -> AuthState {
        AuthState::PendingTwoFactor(self.pending.clone())
    }
}

impl StateSnapshot for AuthenticatedPreBootstrap {
    fn snapshot(&self) -> AuthState {
        AuthState::AuthenticatedPreBootstrap(self.session.clone())
    }
}

impl StateSnapshot for Authenticated {
    fn snapshot(&self) -> AuthState {
        AuthState::Authenticated(self.session.clone())
    }
}
