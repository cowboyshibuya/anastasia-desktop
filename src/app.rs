use std::{collections::BTreeMap, sync::Arc, time::Duration};

use anastasia_harness_api::{ApiEvent, ApiRequest, HistoryMessage, SessionInfo, UserAnswers, UserQuestion};
use gpui::{
    App, Context, Entity, IntoElement, ListAlignment, ListState, Render, Subscription, Task,
    KeyDownEvent, Window, div, list, prelude::*, px,
};

use crate::{harness::{Harness, HarnessEvent}, input::{ComposerEvent, ComposerInput}, theme::Theme};

struct Questions {
    session_id: String,
    request_id: String,
    items: Vec<UserQuestion>,
    index: usize,
    answers: UserAnswers,
    submitting: bool,
}

pub struct Desktop {
    harness: Harness,
    connected: bool,
    notice: String,
    sessions: Vec<SessionInfo>,
    selected: Option<String>,
    history: Arc<Vec<HistoryMessage>>,
    stream: String,
    transcript_state: ListState,
    composer: Entity<ComposerInput>,
    answer_input: Entity<ComposerInput>,
    questions: Option<Questions>,
    question_focus_pending: bool,
    planning: bool,
    models: Vec<String>,
    current_model: Option<String>,
    show_models: bool,
    permission_level: &'static str,
    _subscriptions: Vec<Subscription>,
    _pump: Option<Task<()>>,
}

impl Desktop {
    pub fn new(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let view = cx.new(|cx| {
            let composer = cx.new(|cx| ComposerInput::new(window, cx).placeholder("Ask Anastasia…"));
            let answer_input = cx.new(|cx| ComposerInput::new(window, cx).placeholder("Or type an answer…"));
            let composer_sub = cx.subscribe(&composer, |this: &mut Self, _, event: &ComposerEvent, cx| {
                if let ComposerEvent::Submit(prompt) | ComposerEvent::SubmitSteer(prompt) = event {
                    this.send_message(prompt.clone(), cx);
                }
            });
            let answer_sub = cx.subscribe(&answer_input, |this: &mut Self, _, event: &ComposerEvent, cx| {
                if let ComposerEvent::Submit(answer) | ComposerEvent::SubmitSteer(answer) = event {
                    this.set_custom_answer(answer.clone(), cx);
                }
            });
            Self {
                harness: Harness::start(), connected: false, notice: "Connecting to engine…".into(),
                sessions: Vec::new(), selected: None, history: Arc::new(Vec::new()),
                stream: String::new(), transcript_state: ListState::new(0, ListAlignment::Bottom, px(400.0)),
                composer, answer_input, questions: None, question_focus_pending: false, planning: false,
                models: Vec::new(), current_model: None, show_models: false,
                permission_level: "restricted", _subscriptions: vec![composer_sub, answer_sub], _pump: None,
            }
        });
        view.update(cx, |this, cx| this.start_pump(cx));
        view
    }

    fn start_pump(&mut self, cx: &mut Context<Self>) {
        let receiver = self.harness.events.clone();
        self._pump = Some(cx.spawn(async move |this, cx| {
            while let Ok(first) = receiver.recv().await {
                // The stream sleeps only after an event. Idle windows request no redraws.
                smol::Timer::after(Duration::from_millis(120)).await;
                let mut batch = vec![first];
                while let Ok(event) = receiver.try_recv() { batch.push(event); }
                if this.update(cx, |view, cx| view.apply_events(batch, cx)).is_err() { break; }
            }
        }));
    }

    fn request(&mut self, request: ApiRequest) {
        if let Err(error) = self.harness.send(request) { self.notice = error; }
    }

    fn apply_events(&mut self, batch: Vec<HarnessEvent>, cx: &mut Context<Self>) {
        for event in batch {
            match event {
                HarnessEvent::Connected => {
                    self.connected = true;
                    self.notice = "Connected".into();
                    self.request(ApiRequest::ListSessions { include_archived: false, limit: Some(100) });
                    if let Some(id) = self.selected.clone() {
                        self.request(ApiRequest::AttachSession { session_id: id });
                    }
                }
                HarnessEvent::Disconnected(error) => {
                    self.connected = false;
                    self.questions = None;
                    self.question_focus_pending = false;
                    self.notice = format!("Disconnected: {error}");
                }
                HarnessEvent::Frame(frame) => self.apply_frame(frame.event),
            }
        }
        cx.notify();
    }

    fn apply_frame(&mut self, event: ApiEvent) {
        match event {
            ApiEvent::Sessions { sessions } => {
                let active = self.selected.as_ref().and_then(|id|
                    self.sessions.iter().find(|session| &session.session_id == id).cloned());
                self.sessions = sessions;
                if let Some(active) = active
                    && !self.sessions.iter().any(|session| session.session_id == active.session_id) {
                    self.sessions.insert(0, active);
                }
                if self.selected.is_none() {
                    if let Some(session) = self.sessions.first() {
                        self.attach(session.session_id.clone());
                    }
                }
            }
            ApiEvent::Attached { session } => {
                let id = session.session_id.clone();
                if !self.sessions.iter().any(|item| item.session_id == id) {
                    self.sessions.insert(0, session);
                }
                self.selected = Some(id.clone());
                self.history = Arc::new(Vec::new());
                self.transcript_state.reset(0);
                self.stream.clear();
                self.questions = None;
                self.question_focus_pending = false;
                self.request(ApiRequest::EnableQuestions { session_id: id.clone() });
                self.request(ApiRequest::GetHistory { session_id: id.clone() });
                self.request(ApiRequest::ListModels { session_id: id });
                self.request(ApiRequest::ListSessions { include_archived: false, limit: Some(100) });
            }
            ApiEvent::History { session_id, messages, .. } if self.selected.as_deref() == Some(&session_id) => {
                self.history = Arc::new(messages);
                self.transcript_state.reset(self.history.len());
            }
            ApiEvent::TextDelta { session_id, text } if self.selected.as_deref() == Some(&session_id) => self.stream.push_str(&text),
            ApiEvent::TurnDone { session_id } if self.selected.as_deref() == Some(&session_id) => {
                if !self.stream.is_empty() {
                    Arc::make_mut(&mut self.history).push(HistoryMessage { role: "assistant".into(), content: std::mem::take(&mut self.stream) });
                    self.transcript_state.reset(self.history.len());
                }
                self.notice = "Ready".into();
            }
            ApiEvent::QuestionRequest { session_id, request_id, questions, .. } if self.selected.as_deref() == Some(&session_id) => {
                self.questions = Some(Questions { session_id, request_id, items: questions, index: 0, answers: BTreeMap::new(), submitting: false });
                self.question_focus_pending = true;
                self.notice = "Answer the agent’s question".into();
            }
            ApiEvent::QuestionClosed { session_id, request_id } => {
                if self.questions.as_ref().is_some_and(|q| q.session_id == session_id && q.request_id == request_id) {
                    self.questions = None;
                    self.question_focus_pending = false;
                }
            }
            ApiEvent::PlanningState { session_id, planning, .. } if self.selected.as_deref() == Some(&session_id) => self.planning = planning,
            ApiEvent::Models { session_id, models, current } if self.selected.as_deref() == Some(&session_id) => {
                self.models = models; self.current_model = current;
            }
            ApiEvent::ModelInfo { session_id, model, .. } if self.selected.as_deref() == Some(&session_id) => self.current_model = model,
            ApiEvent::SessionStatus { session_id, status } if self.selected.as_deref() == Some(&session_id) => self.notice = status,
            ApiEvent::MessageAccepted { session_id } if self.selected.as_deref() == Some(&session_id) => {
                self.request(ApiRequest::GetHistory { session_id });
                self.notice = "Sent".into();
            }
            ApiEvent::Error { message, .. } => {
                if let Some(q) = &mut self.questions { q.submitting = false; }
                self.notice = message;
            }
            _ => {}
        }
    }

    fn attach(&mut self, id: String) {
        if !self.connected { return; }
        self.selected = Some(id.clone());
        self.request(ApiRequest::AttachSession { session_id: id });
    }

    fn new_session(&mut self) {
        if self.connected { self.request(ApiRequest::CreateSession { working_dir: None }); }
    }

    fn send_message(&mut self, prompt: String, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else {
            self.composer.update(cx, |input, cx| input.set_content(prompt, cx));
            self.notice = "Create a session first".into(); cx.notify(); return;
        };
        if !self.connected || self.questions.is_some() {
            self.composer.update(cx, |input, cx| input.set_content(prompt, cx));
            self.notice = if self.questions.is_some() { "Answer the question first" } else { "Engine is disconnected; your draft is safe" }.into();
            cx.notify(); return;
        }
        if let Err(error) = self.harness.send(ApiRequest::SendMessage { session_id: id, content: prompt.clone(), images: vec![], no_reply: false }) {
            self.composer.update(cx, |input, cx| input.set_content(prompt, cx));
            self.notice = error;
        } else { self.notice = "Sending…".into(); }
        cx.notify();
    }

    fn toggle_plan(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.selected.clone() {
            self.request(ApiRequest::SetPlanning { session_id: id, planning: !self.planning, goal: None });
            cx.notify();
        }
    }

    fn cancel_turn(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.selected.clone() { self.request(ApiRequest::Cancel { session_id: id }); cx.notify(); }
    }

    fn set_model(&mut self, model: String, cx: &mut Context<Self>) {
        if let Some(id) = self.selected.clone() {
            self.request(ApiRequest::SetModel { session_id: id, model });
            self.show_models = false;
            cx.notify();
        }
    }

    fn set_permissions(&mut self, level: &'static str, cx: &mut Context<Self>) {
        if let Some(id) = self.selected.clone() {
            self.request(ApiRequest::SetPermissions { session_id: id, level: level.into() });
            self.permission_level = level;
            cx.notify();
        }
    }

    fn select_answer(&mut self, label: String, cx: &mut Context<Self>) {
        let Some(q) = &mut self.questions else { return; };
        let Some(item) = q.items.get(q.index) else { return; };
        let values = q.answers.entry(item.id.clone()).or_default();
        if item.multi_select {
            if let Some(index) = values.iter().position(|value| value == &label) { values.remove(index); }
            else { values.push(label); }
        } else {
            values.clear(); values.push(label);
        }
        cx.notify();
    }

    fn set_custom_answer(&mut self, text: String, cx: &mut Context<Self>) {
        let text = text.trim();
        if text.is_empty() { return; }
        let Some(q) = &mut self.questions else { return; };
        if let Some(item) = q.items.get(q.index) {
            q.answers.insert(item.id.clone(), vec![text.to_owned()]);
            q.index += 1;
            self.answer_input.update(cx, |input, cx| input.clear(cx));
            cx.notify();
        }
    }

    fn advance_question(&mut self, cx: &mut Context<Self>) {
        let Some(q) = &mut self.questions else { return; };
        if let Some(item) = q.items.get(q.index) {
            let text = self.answer_input.read(cx).content().trim().to_owned();
            if !text.is_empty() { q.answers.insert(item.id.clone(), vec![text]); }
            if q.answers.get(&item.id).is_none_or(Vec::is_empty) {
                self.notice = "Choose an answer or enter your own".into(); cx.notify(); return;
            }
            self.answer_input.update(cx, |input, cx| input.clear(cx));
            q.index += 1;
            cx.notify();
        } else if !q.submitting && q.answers.len() == q.items.len() {
            let request = ApiRequest::QuestionResponse {
                session_id: q.session_id.clone(), request_id: q.request_id.clone(), answers: q.answers.clone()
            };
            match self.harness.send(request) {
                Ok(_) => { q.submitting = true; self.notice = "Submitting answers…".into(); }
                Err(error) => self.notice = error,
            }
            cx.notify();
        }
    }

    fn back_question(&mut self, cx: &mut Context<Self>) {
        let Some(q) = &mut self.questions else { return; };
        if q.submitting { return; }
        if q.index == 0 {
            let request = ApiRequest::QuestionCancel { session_id: q.session_id.clone(), request_id: q.request_id.clone() };
            self.questions = None;
            self.request(request);
        } else { q.index -= 1; }
        self.answer_input.update(cx, |input, cx| input.clear(cx));
        cx.notify();
    }
}

impl Render for Desktop {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.question_focus_pending {
            self.question_focus_pending = false;
            window.focus(&self.answer_input.read(cx).focus(), cx);
        }
        let theme = Theme::current(cx);
        let rows = self.history.clone();
        let transcript = list(self.transcript_state.clone(), move |index, _, _| {
            let Some(message) = rows.get(index) else { return div().into_any_element(); };
            let user = message.role == "user";
            div().w_full().px(px(20.0)).py(px(14.0))
                .border_b_1().border_color(theme.border)
                .child(div().text_size(px(11.0)).text_color(if user { theme.accent } else { theme.text_tertiary })
                    .child(if user { "YOU" } else { "ANASTASIA" }))
                .child(div().pt(px(8.0)).text_size(px(14.0)).line_height(px(22.0)).text_color(theme.text).child(message.content.clone()))
                .into_any_element()
        }).flex_1().min_h_0();
        let sessions = self.sessions.iter().enumerate().map(|(index, session)| {
            let id = session.session_id.clone();
            let selected = self.selected.as_deref() == Some(id.as_str());
            let title = session.title.clone().unwrap_or_else(|| id.chars().take(12).collect());
            div().id(("session", index)).tab_index(0).focus_visible(|s| s.border_color(theme.accent))
                .px(px(12.0)).py(px(9.0)).rounded(px(8.0))
                .bg(if selected { theme.overlay_strong } else { theme.sidebar })
                .text_size(px(12.0)).text_color(if selected { theme.text } else { theme.text_secondary })
                .cursor_pointer().child(title)
                .on_click(cx.listener({ let id = id.clone(); move |this, _, _, cx| { this.attach(id.clone()); cx.notify(); } }))
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") { this.attach(id.clone()); cx.notify(); }
                }))
        }).collect::<Vec<_>>();
        let model_buttons = self.models.iter().enumerate().map(|(index, model)| {
            let selected = self.current_model.as_deref() == Some(model);
            let name = model.clone();
            div().id(("model", index)).tab_index(0).focus_visible(|s| s.border_color(theme.accent))
                .px(px(10.0)).py(px(5.0)).rounded(px(6.0))
                .bg(if selected { theme.overlay_strong } else { theme.surface })
                .text_size(px(11.0)).child(model.clone())
                .on_click(cx.listener({ let name = name.clone(); move |this, _, _, cx| this.set_model(name.clone(), cx) }))
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") { this.set_model(name.clone(), cx); }
                }))
        }).collect::<Vec<_>>();
        let question_panel = self.questions.as_ref().map(|q| {
            let review = q.index >= q.items.len();
            let header = if review { "Review answers".to_string() } else { format!("Question {} of {}", q.index + 1, q.items.len()) };
            let options = q.items.get(q.index).map(|item| {
                item.options.iter().enumerate().map(|(index, option)| {
                    let label = option.label.clone();
                    let selected = q.answers.get(&item.id).is_some_and(|values| values.contains(&label));
                    div().id(("answer", index)).tab_index(0).focus_visible(|s| s.border_color(theme.accent))
                        .p(px(10.0)).rounded(px(7.0)).border_1()
                        .border_color(if selected { theme.accent } else { theme.border })
                        .bg(if selected { theme.overlay_strong } else { theme.surface })
                        .cursor_pointer()
                        .child(div().text_size(px(13.0)).child(option.label.clone()))
                        .child(div().text_size(px(11.0)).text_color(theme.text_secondary).child(option.description.clone()))
                        .on_click(cx.listener({ let label = label.clone(); move |this, _, _, cx| this.select_answer(label.clone(), cx) }))
                        .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                            if matches!(event.keystroke.key.as_str(), "enter" | "space") { this.select_answer(label.clone(), cx); }
                        }))
                }).collect::<Vec<_>>()
            }).unwrap_or_default();
            div().absolute().inset_0().bg(theme.canvas).p(px(32.0)).flex().flex_col().gap(px(12.0))
                .child(div().text_size(px(12.0)).text_color(theme.accent).child(header))
                .child(if review {
                    div().flex().flex_col().gap(px(9.0)).children(q.items.iter().map(|item| {
                        let answer = q.answers.get(&item.id).map(|values| values.join(", ")).unwrap_or_default();
                        div().text_size(px(13.0)).child(format!("{}: {}", item.header, answer))
                    }).collect::<Vec<_>>())
                } else {
                    div().text_size(px(18.0)).child(q.items[q.index].question.clone())
                })
                .when(!review, |panel| panel.child(div().text_size(px(11.0)).text_color(theme.text_secondary)
                    .child(if q.items[q.index].multi_select { "Select any options, or type your own answer" } else { "Select one option, or type your own answer" })))
                .children(options)
                .when(!review, |panel| panel.child(div().min_h(px(70.0)).child(self.answer_input.clone())))
                .child(div().flex().gap(px(12.0))
                    .child(div().id("question-back").tab_index(0).focus_visible(|s| s.border_color(theme.accent))
                        .p(px(10.0)).border_1().border_color(theme.border).cursor_pointer().child(if q.index == 0 { "Cancel" } else { "Back" })
                        .on_click(cx.listener(|this, _, _, cx| this.back_question(cx)))
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                            if matches!(event.keystroke.key.as_str(), "enter" | "space") { this.back_question(cx); }
                        })))
                    .child(div().id("question-next").tab_index(0).focus_visible(|s| s.border_color(theme.accent))
                        .p(px(10.0)).bg(theme.accent).text_color(theme.on_inverse).cursor_pointer()
                        .child(if review { "Submit" } else { "Next" })
                        .on_click(cx.listener(|this, _, _, cx| this.advance_question(cx)))
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                            if matches!(event.keystroke.key.as_str(), "enter" | "space") { this.advance_question(cx); }
                        }))))
        });
        div().size_full().bg(theme.canvas).text_color(theme.text).flex()
            .child(div().w(px(245.0)).h_full().bg(theme.sidebar).border_r_1().border_color(theme.border)
                .flex().flex_col().p(px(14.0)).gap(px(10.0))
                .child(div().text_size(px(18.0)).text_color(theme.accent).child("Anastasia"))
                .child(div().id("new-session").tab_index(0).focus_visible(|s| s.border_color(theme.accent))
                    .p(px(9.0)).rounded(px(7.0)).bg(theme.raised).cursor_pointer().child("+ New session")
                    .on_click(cx.listener(|this, _, _, cx| { this.new_session(); cx.notify(); })))
                .child(div().id("sessions").flex_1().min_h_0().overflow_y_scroll().children(sessions)))
            .child(div().flex_1().min_w_0().h_full().relative().flex().flex_col()
                .child(div().px(px(18.0)).py(px(12.0)).border_b_1().border_color(theme.border)
                    .flex().items_center().gap(px(12.0))
                    .child(div().id("plan-toggle").tab_index(0).focus_visible(|s| s.border_color(theme.accent))
                        .cursor_pointer().text_size(px(12.0)).text_color(if self.planning { theme.accent } else { theme.text_secondary })
                        .child(if self.planning { "Plan mode" } else { "Build mode" })
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_plan(cx))))
                    .child(div().id("model-toggle").tab_index(0).focus_visible(|s| s.border_color(theme.accent))
                        .cursor_pointer().text_size(px(11.0)).child(self.current_model.clone().unwrap_or("Choose model".into()))
                        .on_click(cx.listener(|this, _, _, cx| { this.show_models = !this.show_models; cx.notify(); })))
                    .child(div().flex_1())
                    .child(div().text_size(px(11.0)).text_color(if self.connected { theme.success } else { theme.danger }).child(self.notice.clone())))
                .when(self.show_models, |root| root.child(div().p(px(10.0)).flex().flex_wrap().gap(px(5.0)).children(model_buttons)))
                .child(transcript)
                .when(!self.stream.is_empty(), |root| root.child(div().px(px(20.0)).py(px(12.0)).text_size(px(14.0)).child(self.stream.clone())))
                .child(div().p(px(14.0)).border_t_1().border_color(theme.border)
                    .child(div().rounded(px(12.0)).bg(theme.composer).p(px(12.0)).min_h(px(85.0)).child(self.composer.clone()))
                    .child(div().pt(px(7.0)).flex().gap(px(12.0)).text_size(px(11.0)).text_color(theme.text_tertiary)
                        .child("Enter sends · Shift+Enter adds a line")
                        .child(div().id("stop-turn").tab_index(0).focus_visible(|s| s.border_color(theme.accent))
                            .cursor_pointer().child("Stop")
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_turn(cx))))
                        .child(div().id("permissions").tab_index(0).focus_visible(|s| s.border_color(theme.accent))
                            .cursor_pointer().child(format!("Permissions: {}", self.permission_level))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let next = match this.permission_level { "restricted" => "auto", "auto" => "full", _ => "restricted" };
                                this.set_permissions(next, cx);
                            })))))
                .when_some(question_panel, |root, panel| root.child(panel)))
    }
}
