//! 内网版 AI 对话页：OpenAI 兼容接口（自定义地址 / Key / 模型）。
//!
//! - 设置在「设置 → AI 对话」里配置，保存在 `settings.txt`（`ai_url` / `ai_key` / `ai_model`）。
//! - 对话支持把文件直接拖到岛上：文本文件会作为附件内容随消息一起发送，
//!   二进制文件只附上文件名与大小说明。
//! - 请求在后台线程执行，界面保持流畅；等待期间输入框禁用。

use crate::app::{App, Cmd, CHAT_H};
use crate::paint::{capsule, Card};
use crate::theme::{self, size, W};
use eframe::egui::{self, Color32};
use std::path::Path;

/// 单条附件：文件名 + 提取出的文本内容。
#[derive(Clone, Debug)]
pub struct Attachment {
    pub name: String,
    pub content: String,
    /// 内容是否因为太大被截断。
    pub truncated: bool,
}

/// 单条消息：`role` 为 `"user"` 或 `"assistant"`。
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Default)]
pub(crate) struct ChatState {
    pub messages: Vec<Message>,
    pub input: String,
    pub attachments: Vec<Attachment>,
    pub busy: bool,
    pub error: String,
    /// 输入框是否已请求过焦点（避免每帧抢焦点）。
    pub focused: bool,
    pub rx: Option<std::sync::mpsc::Receiver<Result<String, String>>>,
}

const MAX_ATTACHMENT_CHARS: usize = 16_000;
const MAX_HISTORY: usize = 40;

impl ChatState {
    fn ready(&self) -> bool { !self.busy }
}

impl App {
    /// 把拖进来的文件变成对话附件。
    pub(crate) fn chat_attach(&mut self, files: Vec<std::path::PathBuf>) {
        for path in files {
            let name=path.file_name().map(|n|n.to_string_lossy().into_owned()).unwrap_or_default();
            if name.is_empty() {continue;}
            let (content,truncated)=read_attachment(&path);
            if self.chat.attachments.iter().any(|a|a.name==name) {continue;}
            self.chat.attachments.push(Attachment{name,content,truncated});
            if self.chat.attachments.len()>=6 {self.say("附件最多 6 个");break;}
        }
    }

    /// 发送当前输入与附件；接口没配置时直接跳到设置页。
    pub(crate) fn chat_send(&mut self) -> bool {
        if !self.chat.ready() {return false;}
        let text=self.chat.input.trim().to_string();
        if text.is_empty() && self.chat.attachments.is_empty() {return false;}
        let cfg=(self.settings.ai_url.trim().to_string(),self.settings.ai_key.trim().to_string(),self.settings.ai_model.trim().to_string());
        if cfg.0.is_empty() || cfg.1.is_empty() || cfg.2.is_empty() {
            self.say("先在设置 → AI 对话里配置接口地址、Key 和模型");
            self.open_ai_settings();
            return true;
        }
        let mut content=text;
        for a in &self.chat.attachments {
            content.push_str(&format!("\n\n【附件：{}{}】\n{}",a.name,if a.truncated{"（已截断）"}else{""},a.content));
        }
        self.chat.messages.push(Message{role:"user".into(),content});
        self.chat.input.clear();
        self.chat.attachments.clear();
        self.chat.error.clear();
        while self.chat.messages.len()>MAX_HISTORY {self.chat.messages.remove(0);}
        let history=self.chat.messages.clone();
        let (tx,rx)=std::sync::mpsc::channel();
        self.chat.busy=true;
        self.chat.rx=Some(rx);
        std::thread::spawn(move || { let _=tx.send(ai_request(&cfg,&history)); });
        true
    }

    /// 每帧检查后台请求是否完成。
    pub(crate) fn chat_poll(&mut self) {
        let Some(rx)=&self.chat.rx else {return;};
        let mut done=None;
        while let Ok(result)=rx.try_recv() {done=Some(result);}
        let Some(result)=done else {return;};
        self.chat.rx=None;
        self.chat.busy=false;
        match result {
            Ok(text)=>if text=="连接成功" && self.chat.messages.is_empty() {
                self.chat.error.clear();
                self.say("AI 接口连接成功");
            } else if !text.trim().is_empty() {
                self.chat.messages.push(Message{role:"assistant".into(),content:text});
            },
            Err(error)=>{self.chat.error=error.clone();self.chat.messages.push(Message{role:"assistant".into(),content:error});},
        }
    }

    /// AI 对话页主体。面板尺寸 419×CHAT_H。
    pub(crate) fn page_ai(&mut self, c: &Card, _env: &crate::app::Env, cmds: &mut Vec<Cmd>) {
        self.chat_poll();
        let configured=!self.settings.ai_url.trim().is_empty() && !self.settings.ai_key.trim().is_empty() && !self.settings.ai_model.trim().is_empty();
        if !configured && self.chat.messages.is_empty() {
            c.text(42.0,62.0,"还没有配置 AI 接口",13.0,W::Bold,theme::WHITE,crate::paint::H::Left);
            let hint=c.fit("在设置 → AI 对话里填入内网大模型服务的地址、API Key 和模型名",10.5,W::Regular,344.0);
            c.text(42.0,80.0,&hint,10.5,W::Regular,theme::GRAY,crate::paint::H::Left);
            let (go,gc)=capsule(c,42.0,92.0,84.0,26.0,"ai-setup",theme::BTN,theme::BTN_HOVER);
            gc.text(58.0,109.0,"去配置",size::BODY,W::Regular,theme::WHITE,crate::paint::H::Left);
            if go.clicked() {self.open_ai_settings();}
            return;
        }

        // 附件条：显示在消息区上方
        let mut attach_y=34.0;
        if !self.chat.attachments.is_empty() {
            for (i,a) in self.chat.attachments.iter().enumerate() {
                let x=36.0+ (i%3) as f32*126.0;
                let y=attach_y+ (i/3) as f32*20.0;
                c.fill(x,y,118.0,16.0,5.0,theme::CARD);
                c.text(x+6.0,y+11.5,&c.fit(&format!("📎 {}",a.name),9.5,W::Regular,96.0),9.5,W::Regular,Color32::from_gray(210),crate::paint::H::Left);
                let remove=c.hit(x+104.0,y+2.0,12.0,12.0,("ai-attach-remove",i));
                c.text(x+110.0,y+11.0,"×",10.0,W::Regular,if remove.hovered(){theme::WHITE}else{theme::GRAY},crate::paint::H::Center);
                if remove.clicked() {self.chat.attachments.remove(i);break;}
            }
            attach_y+=20.0*(1+(self.chat.attachments.len()-1)/3) as f32+4.0;
        }

        // 消息区（滚动）
        let top=attach_y.max(34.0);
        let area=(36.0,top,347.0,150.0_f32.min(206.0-top));
        if area.3>24.0 {
            let mut content=egui::Ui::new(c.ui.ctx().clone(),egui::Id::new(("ai-chat",c.live)),
                egui::UiBuilder::new().max_rect(c.rect(area.0,area.1,area.2,area.3)).layer_id(c.painter.layer_id()).style(c.ui.style().clone()));
            content.set_clip_rect(c.rect(area.0,area.1,area.2,area.3).intersect(c.painter.clip_rect()));
            content.set_opacity(c.painter.opacity());
            if !c.live {content.disable();}
            content.spacing_mut().item_spacing.y=6.0*c.s;
            content.spacing_mut().scroll.floating=false;
            content.spacing_mut().scroll.bar_width=4.0*c.s;
            let count=self.chat.messages.len();
            let scroll=egui::ScrollArea::vertical().id_salt(("ai-chat-scroll",count)).max_height(area.3*c.s).auto_shrink([false,false]);
            scroll.show(&mut content,|ui|{
                for msg in &self.chat.messages {
                    let mine=msg.role=="user";
                    let text=if mine {&msg.content} else {&msg.content};
                    let label=egui::RichText::new(text)
                        .font(c.font(11.5,W::Regular))
                        .color(if mine {Color32::from_gray(235)}else{Color32::from_gray(200)});
                    let (bubble,align)=if mine {(theme::BLUE,1.0)}else{(Color32::from_gray(40),0.0)};
                    egui::Frame::new().fill(bubble).corner_radius(8.0).inner_margin(egui::Margin::symmetric(8,5)).show(ui,|ui|{
                        ui.set_max_width((area.2*0.82)*c.s);
                        ui.with_layout(if mine {egui::Layout::right_to_left(egui::Align::Center)}else{egui::Layout::left_to_right(egui::Align::Center)},|ui|{
                            ui.add(egui::Label::new(label).wrap());
                        });
                    });
                    let _=align;
                }
                if self.chat.busy {
                    ui.label(egui::RichText::new("正在思考…").font(c.font(10.5,W::Regular)).color(theme::DIM));
                }
            });
        }
        if self.chat.messages.is_empty() && !self.chat.busy && self.chat.error.is_empty() && configured {
            c.text(210.0,100.0,"把文件拖到岛上，或直接在下面提问",11.0,W::Regular,theme::DIM,crate::paint::H::Center);
        }

        // 输入区
        let input_y=CHAT_H-44.0;
        c.fill(36.0,input_y,347.0,30.0,9.0,theme::CARD);
        let rect=c.rect(46.0,input_y+5.0,280.0,20.0);
        let mut buf=std::mem::take(&mut self.chat.input);
        let te=egui::TextEdit::singleline(&mut buf)
            .id(egui::Id::new("ai-input"))
            .frame(false)
            .margin(egui::Margin::ZERO)
            .text_color(theme::WHITE)
            .font(c.font(12.0,W::Regular))
            .hint_text(egui::RichText::new(if self.chat.busy {"等待回复中…"}else{"问点什么？回车发送"}).color(theme::DIM).font(c.font(12.0,W::Regular)))
            .desired_width(rect.width());
        let mut input_ui=egui::Ui::new(c.ui.ctx().clone(),egui::Id::new(("ai-input-ui",c.live)),
            egui::UiBuilder::new().max_rect(rect).layer_id(c.painter.layer_id()).style(c.ui.style().clone()));
        input_ui.set_clip_rect(rect.intersect(c.painter.clip_rect()));
        input_ui.set_opacity(c.painter.opacity());
        if !c.live {input_ui.disable();}
        let resp=input_ui.put(rect,te);
        let first_focus=!self.chat.focused;
        self.chat.input=buf;
        let lost_window=c.ui.ctx().input(|i|i.events.iter().any(|event|matches!(event,egui::Event::WindowFocused(false))));
        if first_focus {
            self.chat.focused=true;
            resp.request_focus();
            cmds.push(Cmd::Focus);
        }
        if !self.chat.busy && self.chat.input.trim().is_empty() && self.chat.attachments.is_empty() && (resp.clicked_elsewhere()||lost_window) {
            resp.surrender_focus();
            self.chat.focused=false;
        }
        let enter=c.ui.ctx().input(|i|i.key_pressed(egui::Key::Enter));
        if enter && resp.has_focus() && self.chat_send() {
            self.chat.focused=true;
        }

        // 发送 / 停止按钮
        let (btn,bc)=capsule(c,334.0,input_y+2.0,44.0,26.0,"ai-send",theme::BTN,theme::BTN_HOVER);
        bc.text(356.0,input_y+19.0,if self.chat.busy {"…"}else{"发送"},size::BODY,W::Regular,if self.chat.ready()||true{theme::WHITE}else{theme::GRAY},crate::paint::H::Center);
        if btn.clicked() {self.chat_send();}
        if !self.chat.error.is_empty() {
            c.text(46.0,input_y-6.0,&c.fit(&self.chat.error,9.0,W::Regular,320.0),9.0,W::Regular,theme::RED_TOP,crate::paint::H::Left);
        }
        c.text(394.0,input_y-6.0,&c.fit(&self.settings.ai_model,9.0,W::Regular,90.0),9.0,W::Regular,theme::DIM,crate::paint::H::Right);
    }
}

/// 读取附件：文本原样（超长截断），二进制只保留说明。
fn read_attachment(path: &Path) -> (String, bool) {
    let bytes=match std::fs::read(path) {
        Ok(bytes)=>bytes,
        Err(e)=>return (format!("（无法读取文件：{e}）"),false),
    };
    let binary=bytes.len()>2*1024*1024 || bytes.contains(&0);
    if binary {
        return (format!("（二进制文件，{}；无法直接读取内容，仅附上文件名供参考）",crate::model::human_size(bytes.len() as u64)),false);
    }
    let mut text=String::from_utf8_lossy(&bytes).into_owned();
    if text.chars().count()>MAX_ATTACHMENT_CHARS {
        text=text.chars().take(MAX_ATTACHMENT_CHARS).collect();
        return (text,true);
    }
    (text,false)
}

/// 设置页「测试连接」：只验证地址、Key 与模型可用，不进入对话历史。
pub fn test_connection(cfg: &(String, String, String)) -> Result<String, String> {
    let history = vec![Message { role: "user".into(), content: "ping".into() }];
    ai_request(cfg, &history).map(|_| "连接成功".to_string())
}

/// 调用 OpenAI 兼容接口 `/chat/completions`。
fn ai_request(cfg: &(String,String,String), history: &[Message]) -> Result<String,String> {
    let (url,key,model)=(&cfg.0,&cfg.1,&cfg.2);
    let endpoint=format!("{}/chat/completions",url.trim_end_matches('/'));
    let messages: Vec<_>=history.iter().map(|m|serde_json::json!({
        "role": if m.role=="user" {"user"} else {"assistant"},
        "content": m.content,
    })).collect();
    let body=serde_json::json!({"model":model,"messages":messages,"stream":false});
    let agent=ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(15))
        .timeout_read(std::time::Duration::from_secs(300))
        .timeout(std::time::Duration::from_secs(300))
        .build();
    let response=agent.post(&endpoint)
        .set("Authorization",&format!("Bearer {key}"))
        .set("Content-Type","application/json")
        .send_string(&serde_json::to_string(&body).unwrap_or_default())
        .map_err(|e|match e {
            ureq::Error::Status(code,resp)=>format!("接口返回 {code}：{}",resp.into_string().unwrap_or_default().chars().take(200).collect::<String>()),
            ureq::Error::Transport(t)=>format!("网络错误：{t}"),
        })?;
    let text=response.into_string().map_err(|e|format!("读取响应失败：{e}"))?;
    let json:serde_json::Value=serde_json::from_str(&text).map_err(|e|format!("响应不是合法 JSON：{e}"))?;
    let content=json.pointer("/choices/0/message/content")
        .and_then(|v|v.as_str())
        .map(str::to_string)
        .or_else(||json.get("error").and_then(|e|e.get("message")).and_then(|m|m.as_str()).map(str::to_string))
        .ok_or_else(||format!("响应格式无法识别：{}",text.chars().take(200).collect::<String>()))?;
    Ok(content)
}
