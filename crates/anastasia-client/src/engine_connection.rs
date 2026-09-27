//! Desktop connection to the one shared agent engine.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anastasia_sdk::{
    AnastasiaClient, ApiEvent, ConnectOptions, EventStream, HistoryMessage, LaunchOptions,
    PermissionDecision, SessionInfo, UserAnswers, ensure_runtime,
};
use parking_lot::Mutex;

use crate::driver::{DriverControl, DriverEventSender, DriverHandle};
use anastasia_protocol::model::{
    ActivityKind, DriverEvent, PermissionOption, ProviderResumeCursor, UserInputAnswer,
    UserInputOption, UserInputQuestion,
};

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

    /// Subscribe before the GUI submits a prompt, so its first stream delta
    /// cannot race past the transcript renderer.
    pub fn driver(
        self: &Arc<Self>,
        session_id: &str,
        events: DriverEventSender,
    ) -> anyhow::Result<DriverHandle> {
        let client = self.attached(session_id)?;
        let stream = client.events(Some(session_id));
        let control = Arc::new(EngineDriver {
            engine: Arc::clone(self),
            session_id: session_id.to_string(),
            events: events.clone(),
            closed: AtomicBool::new(false),
        });
        let reader = Arc::clone(&control);
        std::thread::Builder::new()
            .name(format!("anastasia-engine-session-{session_id}"))
            .spawn(move || {
                'read: while !reader.closed.load(Ordering::Acquire) {
                    match stream.next_timeout(Duration::from_millis(100)) {
                        Some(event) => {
                            for event in driver_events(event).into_iter().flatten() {
                                if reader.events.send(event).is_err() {
                                    break 'read;
                                }
                            }
                        }
                        None if client.is_closed() => break,
                        None => {}
                    }
                }
                if !reader.closed.load(Ordering::Acquire) {
                    let _ = reader.events.send(DriverEvent::ProcessExited);
                }
            })?;
        Ok(DriverHandle::from_control(control))
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
        self.control.archive_session(session_id)?;
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

struct EngineDriver {
    engine: Arc<EngineConnection>,
    session_id: String,
    events: DriverEventSender,
    closed: AtomicBool,
}

impl EngineDriver {
    fn report(&self, result: anyhow::Result<()>) {
        if let Err(error) = result {
            let _ = self.events.send(DriverEvent::Error(error.to_string()));
        }
    }
}

impl DriverControl for EngineDriver {
    fn prompt(&self, prompt: String) {
        self.report(self.engine.send_message(&self.session_id, &prompt));
    }

    fn supports_steer(&self) -> bool {
        true
    }

    fn steer(&self, prompt: String) {
        self.report(self.engine.attached(&self.session_id).and_then(|client| {
            client.soft_interrupt(&self.session_id, &prompt, false)?;
            Ok(())
        }));
    }

    fn cancel(&self) {
        self.report(self.engine.cancel(&self.session_id));
    }

    fn respond(&self, request_id: String, option_id: String) {
        let decision = match option_id.as_str() {
            "allow" => PermissionDecision::Allow,
            "allow_always" => PermissionDecision::AllowAlways,
            "deny" => PermissionDecision::Deny,
            _ => {
                self.report(Err(anyhow::anyhow!(
                    "unknown engine permission choice {option_id}"
                )));
                return;
            }
        };
        self.report(
            self.engine
                .respond_to_permission(&self.session_id, &request_id, decision),
        );
    }

    fn respond_user_input(&self, request_id: String, answers: Vec<UserInputAnswer>) {
        if answers.is_empty() {
            self.report(self.engine.cancel_question(&self.session_id, &request_id));
        } else {
            let answers = answers
                .into_iter()
                .map(|answer| (answer.question_id, answer.answers))
                .collect();
            self.report(
                self.engine
                    .respond_to_question(&self.session_id, &request_id, answers),
            );
        }
    }

    fn rollback(&self, _turns: usize) -> anyhow::Result<Option<ProviderResumeCursor>> {
        anyhow::bail!("engine-backed turn rollback is not yet available in the desktop")
    }

    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.report(self.engine.detach_session(&self.session_id));
    }
}

fn driver_events(event: ApiEvent) -> [Option<DriverEvent>; 2] {
    match event {
        ApiEvent::TurnDone { .. } => [
            Some(DriverEvent::TurnFinished {
                success: true,
                summary: None,
            }),
            None,
        ],
        ApiEvent::Error { message, .. } => [
            Some(DriverEvent::Error(message)),
            Some(DriverEvent::TurnFinished {
                success: false,
                summary: None,
            }),
        ],
        event => [driver_event(event), None],
    }
}

fn driver_event(event: ApiEvent) -> Option<DriverEvent> {
    match event {
        ApiEvent::MessageAccepted { .. } => Some(DriverEvent::TurnStarted),
        ApiEvent::TextDelta { text, .. } => Some(DriverEvent::TextDelta(text)),
        ApiEvent::ReasoningDelta { text, .. } => Some(DriverEvent::ReasoningDelta(text)),
        ApiEvent::ToolStart { call_id, name, .. } | ApiEvent::ToolExec { call_id, name, .. } => {
            Some(DriverEvent::Activity {
                id: Some(call_id),
                kind: ActivityKind::from_tool_name(&name),
                title: name,
                detail: None,
                complete: false,
            })
        }
        ApiEvent::ToolDone {
            call_id,
            name,
            output,
            error,
            ..
        } => Some(DriverEvent::Activity {
            id: Some(call_id),
            kind: ActivityKind::from_tool_name(&name),
            title: name,
            detail: Some(error.unwrap_or(output)),
            complete: true,
        }),
        ApiEvent::PermissionRequest {
            request_id,
            tool_name,
            description,
            ..
        } => Some(DriverEvent::Permission {
            request_id,
            title: tool_name,
            detail: description,
            options: vec![
                PermissionOption {
                    id: "allow".into(),
                    label: "Allow once".into(),
                    allow: true,
                },
                PermissionOption {
                    id: "allow_always".into(),
                    label: "Always allow".into(),
                    allow: true,
                },
                PermissionOption {
                    id: "deny".into(),
                    label: "Deny".into(),
                    allow: false,
                },
            ],
        }),
        ApiEvent::QuestionRequest {
            request_id,
            questions,
            ..
        } => Some(DriverEvent::UserInputRequested {
            request_id,
            questions: questions
                .into_iter()
                .map(|question| UserInputQuestion {
                    id: question.id,
                    header: question.header,
                    question: question.question,
                    options: question
                        .options
                        .into_iter()
                        .map(|option| UserInputOption {
                            label: option.label,
                            description: Some(option.description),
                        })
                        .collect(),
                    multi_select: question.multi_select,
                })
                .collect(),
        }),
        ApiEvent::SessionRenamed { display_title, .. } => {
            Some(DriverEvent::AutoTitleUpdated(Some(display_title)))
        }
        _ => None,
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
                && session.title.as_deref() == Some("Smoke test")
                && session.planning
                && session.plan_goal.as_deref() == Some("smoke-test goal"))
    );
    engine.detach_session(&created.session_id).unwrap();
    engine.attach_session(&created.session_id).unwrap();
    assert!(!engine.history(&created.session_id).unwrap().is_empty());
}

#[cfg(test)]
#[test]
fn engine_questions_and_tools_keep_gui_ids_and_choices() {
    let question = anastasia_sdk::UserQuestion {
        id: "scope".into(),
        header: "Scope".into(),
        question: "Which apps?".into(),
        options: vec![anastasia_sdk::UserQuestionOption {
            label: "CLI".into(),
            description: "Terminal client".into(),
        }],
        multi_select: true,
    };
    let Some(DriverEvent::UserInputRequested {
        request_id,
        questions,
    }) = driver_event(ApiEvent::QuestionRequest {
        session_id: "session-1".into(),
        request_id: "request-1".into(),
        tool_call_id: "tool-1".into(),
        questions: vec![question],
    })
    else {
        panic!("question did not reach the GUI");
    };
    assert_eq!(request_id, "request-1");
    assert_eq!(questions[0].id, "scope");
    assert!(questions[0].multi_select);
    assert_eq!(
        questions[0].options[0].description.as_deref(),
        Some("Terminal client")
    );

    let Some(DriverEvent::Activity {
        id, kind, complete, ..
    }) = driver_event(ApiEvent::ToolDone {
        session_id: "session-1".into(),
        call_id: "tool-1".into(),
        name: "apply_patch".into(),
        output: "done".into(),
        error: None,
    })
    else {
        panic!("tool completion did not reach the GUI");
    };
    assert_eq!(id.as_deref(), Some("tool-1"));
    assert_eq!(kind, ActivityKind::FileChange);
    assert!(complete);

    let failed = driver_events(ApiEvent::Error {
        code: anastasia_sdk::api::ErrorCode::Internal,
        message: "provider failed".into(),
    });
    assert!(matches!(failed[0], Some(DriverEvent::Error(_))));
    assert!(matches!(
        failed[1],
        Some(DriverEvent::TurnFinished { success: false, .. })
    ));
}
