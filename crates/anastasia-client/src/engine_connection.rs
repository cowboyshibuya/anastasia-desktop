//! Desktop connection to the one shared agent engine.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anastasia_sdk::{
    AnastasiaClient, ConnectOptions, EventStream, HistoryMessage, LaunchOptions,
    PermissionDecision, SessionInfo, UserAnswers, ensure_runtime,
};
use parking_lot::Mutex;

/// One control connection and one attached connection per live GUI task.
/// The engine API attaches one session per connection.
pub struct EngineConnection {
    control: AnastasiaClient,
    live: Mutex<HashMap<String, AnastasiaClient>>,
}

impl EngineConnection {
    pub fn connect(binary: &Path) -> anyhow::Result<Self> {
        let launch = LaunchOptions {
            binary: Some(binary.to_path_buf()),
            client_name: "anastasia-desktop/0.1".into(),
            env: [("ANASTASIA_CLI_DEFERRED_AUTH_BOOTSTRAP".into(), "1".into())]
                .into_iter()
                .collect(),
            ..LaunchOptions::default()
        };
        ensure_runtime(&launch, &|_| {})?;
        Ok(Self {
            control: AnastasiaClient::connect(connect_options())?,
            live: Mutex::new(HashMap::new()),
        })
    }

    pub fn list_sessions(&self) -> anyhow::Result<Vec<SessionInfo>> {
        Ok(self.control.list_sessions()?)
    }

    pub fn create_session(&self, working_dir: Option<String>) -> anyhow::Result<SessionInfo> {
        let client = AnastasiaClient::connect(connect_options())?;
        let session = client.create_session(working_dir)?;
        client.enable_questions(&session.session_id)?;
        self.live.lock().insert(session.session_id.clone(), client);
        Ok(session)
    }

    pub fn attach_session(&self, session_id: &str) -> anyhow::Result<SessionInfo> {
        let client = AnastasiaClient::connect(connect_options())?;
        let session = client.attach_session(session_id)?;
        client.enable_questions(&session.session_id)?;
        self.live.lock().insert(session.session_id.clone(), client);
        Ok(session)
    }

    pub fn events(&self, session_id: &str) -> anyhow::Result<EventStream> {
        Ok(self.attached(session_id)?.events(Some(session_id)))
    }

    pub fn send_message(&self, session_id: &str, text: &str) -> anyhow::Result<()> {
        self.attached(session_id)?
            .send_message(session_id, text, vec![], None)?;
        Ok(())
    }

    pub fn cancel(&self, session_id: &str) -> anyhow::Result<()> {
        self.attached(session_id)?.cancel(session_id)?;
        Ok(())
    }

    pub fn respond_to_permission(
        &self,
        session_id: &str,
        request_id: &str,
        decision: PermissionDecision,
    ) -> anyhow::Result<()> {
        self.attached(session_id)?
            .respond_to_permission(session_id, request_id, decision)?;
        Ok(())
    }

    pub fn respond_to_question(
        &self,
        session_id: &str,
        request_id: &str,
        answers: UserAnswers,
    ) -> anyhow::Result<()> {
        self.attached(session_id)?
            .respond_to_question(session_id, request_id, answers)?;
        Ok(())
    }

    pub fn cancel_question(&self, session_id: &str, request_id: &str) -> anyhow::Result<()> {
        self.attached(session_id)?
            .cancel_question(session_id, request_id)?;
        Ok(())
    }

    pub fn set_planning(
        &self,
        session_id: &str,
        planning: bool,
        goal: Option<String>,
    ) -> anyhow::Result<()> {
        self.attached(session_id)?
            .set_planning(session_id, planning, goal)?;
        Ok(())
    }

    pub fn set_permissions(&self, session_id: &str, level: &str) -> anyhow::Result<()> {
        self.attached(session_id)?
            .set_permissions(session_id, level)?;
        Ok(())
    }

    pub fn rename_session(&self, session_id: &str, title: &str) -> anyhow::Result<()> {
        self.attached(session_id)?
            .rename_session(session_id, Some(title.to_string()))?;
        Ok(())
    }

    pub fn archive_session(&self, session_id: &str) -> anyhow::Result<()> {
        self.attached(session_id)?.archive_session(session_id)?;
        Ok(())
    }

    pub fn history(&self, session_id: &str) -> anyhow::Result<Vec<HistoryMessage>> {
        Ok(self.attached(session_id)?.get_history(session_id)?)
    }

    pub fn detach_session(&self, session_id: &str) -> anyhow::Result<()> {
        if let Some(client) = self.live.lock().remove(session_id) {
            client.detach_session(session_id)?;
        }
        Ok(())
    }

    fn attached(&self, session_id: &str) -> anyhow::Result<AnastasiaClient> {
        self.live
            .lock()
            .get(session_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("session {session_id} is not attached to this desktop"))
    }
}

fn connect_options() -> ConnectOptions {
    ConnectOptions {
        client_name: "anastasia-desktop/0.1".into(),
        request_timeout: Some(Duration::from_secs(120)),
        ensure_runtime: false,
        ..ConnectOptions::default()
    }
}

pub fn engine_binary() -> anyhow::Result<PathBuf> {
    if let Some(path) = std::env::var_os("ANASTASIA_ENGINE_BIN") {
        return Ok(PathBuf::from(path));
    }
    let executable = std::env::current_exe()?;
    let directory = executable
        .parent()
        .ok_or_else(|| anyhow::anyhow!("desktop executable has no parent directory"))?;
    let name = if cfg!(windows) {
        "anastasia-engine.exe"
    } else {
        "anastasia-engine"
    };
    [
        directory.join(name),
        directory.join("../Resources").join(name),
    ]
    .into_iter()
    .find(|path| path.is_file())
    .ok_or_else(|| {
        anyhow::anyhow!(
            "the bundled Anastasia engine is missing; set ANASTASIA_ENGINE_BIN for development"
        )
    })
}

#[cfg(test)]
#[test]
#[ignore = "requires ANASTASIA_ENGINE_BIN and a private ANASTASIA_CLI_HOME/RUNTIME_DIR"]
fn engine_session_roundtrip() {
    let binary = engine_binary().unwrap();
    let workspace = std::path::PathBuf::from(std::env::var("ANASTASIA_CLI_HOME").unwrap())
        .join("smoke-workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let engine = EngineConnection::connect(&binary).unwrap();
    let created = engine
        .create_session(Some(workspace.display().to_string()))
        .unwrap();
    assert!(engine.history(&created.session_id).unwrap().is_empty());
    engine
        .set_planning(&created.session_id, true, Some("smoke-test goal".into()))
        .unwrap();
    engine
        .rename_session(&created.session_id, "Smoke test")
        .unwrap();
    let _ = engine
        .attached(&created.session_id)
        .unwrap()
        .request(anastasia_sdk::ApiRequest::SendMessage {
            session_id: created.session_id.clone(),
            content: "smoke-test context".into(),
            images: vec![],
            no_reply: true,
        })
        .unwrap();
    assert!(
        engine
            .list_sessions()
            .unwrap()
            .iter()
            .any(|session| session.session_id == created.session_id
                && session.title.as_deref() == Some("Smoke test"))
    );
    engine.detach_session(&created.session_id).unwrap();
    engine.attach_session(&created.session_id).unwrap();
    assert!(!engine.history(&created.session_id).unwrap().is_empty());
}
