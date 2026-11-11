//! 邮件工具（岛仔内网版）：仅保留 MIME/ICS 解析（`mailparse`），
//! 供日程模块解析邮件邀请里的会议（`Meeting` / `meetings_of`）。
//! 内网版不提供 IMAP 收信，账号与凭据相关能力已移除。


pub const SERVICE: &str = "Daozai.Mail";

// ---------------------------------------------------------------- 邮箱类型

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Qq,
    Mail163,
    Mail126,
    Exmail,
    ICloud,
    Gmail,
}

impl Default for Provider {
    fn default() -> Self {
        Provider::Qq
    }
}

impl Provider {
    pub const ALL: [Provider; 6] = [
        Provider::Qq,
        Provider::Mail163,
        Provider::Mail126,
        Provider::Exmail,
        Provider::ICloud,
        Provider::Gmail,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Provider::Qq => "qq",
            Provider::Mail163 => "163",
            Provider::Mail126 => "126",
            Provider::Exmail => "exmail",
            Provider::ICloud => "icloud",
            Provider::Gmail => "gmail",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Provider::Qq => "QQ 邮箱",
            Provider::Mail163 => "163 邮箱",
            Provider::Mail126 => "126 邮箱",
            Provider::Exmail => "腾讯企业邮",
            Provider::ICloud => "iCloud 邮箱",
            Provider::Gmail => "Gmail",
        }
    }

    pub fn host(self) -> &'static str {
        match self {
            Provider::Qq => "imap.qq.com",
            Provider::Mail163 => "imap.163.com",
            Provider::Mail126 => "imap.126.com",
            Provider::Exmail => "imap.exmail.qq.com",
            Provider::ICloud => "imap.mail.me.com",
            Provider::Gmail => "imap.gmail.com",
        }
    }

    pub fn port(self) -> u16 {
        993
    }

    /// 163 / 126 在 LOGIN 之前必须发 RFC 2971 的 `ID`，否则会被判 Unsafe Login。
    pub fn needs_id(self) -> bool {
        matches!(self, Provider::Mail163 | Provider::Mail126)
    }

    /// 设置界面的操作指引。
    pub fn hint(self) -> &'static str {
        match self {
            Provider::Qq => "1. 登录 QQ 邮箱网页版，点右上角「设置」→ 左侧「账号与安全」。\n2. 在新页面选「安全设置」，向下找到「POP3/IMAP/SMTP/Exchange/CardDAV 服务」；未开启时先按提示开启。\n3. 点「生成授权码」，完成验证后复制到下方。这里填写授权码，不是 QQ 密码或邮箱独立密码。",
            Provider::Mail163 | Provider::Mail126 => "1. 登录对应的网易邮箱网页版，点顶部「设置」→「POP3/SMTP/IMAP」。\n2. 确认「IMAP/SMTP 服务」已开启；首次开启时按提示完成验证。\n3. 在「授权密码管理」中点「新增授权密码」，完成验证后复制到下方。填写客户端授权码，不是邮箱登录密码。",
            Provider::Exmail => "1. 登录腾讯企业邮网页版，在「设置」→「收发信设置」检查 IMAP/SMTP 服务。\n2. 常见网页版在「设置」→「邮箱绑定」→「客户端专用密码」→「生成新密码」获取密码；启用安全登录后须使用专用密码。\n3. 企业版本和管理员策略可能不同。找不到入口或服务被禁用时，请让企业管理员确认客户端接入方式，不要照搬 QQ 邮箱的路径。",
            Provider::ICloud => "1. 登录 Apple 账户页面，进入「登录和安全」→「App 专用密码」。\n2. 选择「生成 App 专用密码」，按提示填写名称并完成验证；账户需已开启双重认证。\n3. 邮箱栏填写完整的 iCloud 邮箱地址，下方填写刚生成的 App 专用密码，不是 Apple 账户密码。",
            Provider::Gmail => "1. 登录 Google 账户，进入「安全性与登录」→「您的 Google 账号登录选项」→「开启两步验证」，按提示完成设置。\n2. 打开下方「应用专用密码」页面，输入应用名称（例如：岛仔）并创建，复制生成的 16 位应用专用密码。\n3. 邮箱栏填完整 Gmail 地址，下方填应用专用密码，不是 Google 登录密码。若没有创建入口，请检查组织账号限制、高级保护或仅使用安全密钥的验证设置。",
        }
    }

    pub fn setup_link(self) -> (&'static str, &'static str) {
        match self {
            Provider::Qq => ("打开 QQ 邮箱网页版", "https://mail.qq.com/"),
            Provider::Mail163 => ("打开 163 邮箱网页版", "https://mail.163.com/"),
            Provider::Mail126 => ("打开 126 邮箱网页版", "https://mail.126.com/"),
            Provider::Exmail => ("打开腾讯企业邮网页版", "https://exmail.qq.com/"),
            Provider::ICloud => ("打开 Apple 账户", "https://account.apple.com/"),
            Provider::Gmail => ("打开应用专用密码", "https://myaccount.google.com/apppasswords"),
        }
    }

    pub fn parse(key: &str) -> Self {
        match key {
            "163" => Provider::Mail163,
            "126" => Provider::Mail126,
            "exmail" => Provider::Exmail,
            "icloud" => Provider::ICloud,
            "gmail" => Provider::Gmail,
            _ => Provider::Qq,
        }
    }
}

// ---------------------------------------------------------------- 账号与状态

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub email: String,
    pub provider: Provider,
    /// 是否监控这个账号（设置里可逐个开关）。
    pub enabled: bool,
}

pub fn key(account: &Account) -> String {
    account.email.trim().to_ascii_lowercase()
}

fn accounts_path() -> std::path::PathBuf {
    crate::model::config_dir().join("mail-accounts.json")
}

pub fn load_accounts() -> Vec<Account> {
    match std::fs::read(accounts_path()) {
        Ok(raw) => decode_accounts(&raw),
        Err(_) => Vec::new(),
    }
}

/// 纯解析：账号清单里只有邮箱、类型、开关，没有任何密码字段。
pub fn decode_accounts(raw: &[u8]) -> Vec<Account> {
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(raw) else {
        return Vec::new();
    };
    json.as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| {
            let email = a.get("email")?.as_str()?.trim().to_string();
            if email.is_empty() {
                return None;
            }
            Some(Account {
                provider: Provider::parse(a.get("provider").and_then(|p| p.as_str()).unwrap_or("qq")),
                enabled: a.get("enabled").and_then(|e| e.as_bool()).unwrap_or(true),
                email,
            })
        })
        .collect()
}

pub fn encode_accounts(items: &[Account]) -> Vec<u8> {
    let list: Vec<_> = items
        .iter()
        .map(|a| serde_json::json!({"email": a.email, "provider": a.provider.key(), "enabled": a.enabled}))
        .collect();
    serde_json::to_vec(&list).unwrap_or_default()
}

pub fn save_accounts(items: &[Account]) {
    let data = encode_accounts(items);
    if data.is_empty() {
        return;
    }
    let path = accounts_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = crate::model::atomic_write(&path, &data);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Idle,
    Connecting,
    Ready,
    /// 授权码缺失或失效。
    Auth(String),
    Failed(String),
}

impl Status {
    pub fn label(&self) -> String {
        match self {
            Status::Idle => "未连接".into(),
            Status::Connecting => "连接中…".into(),
            Status::Ready => "已连接".into(),
            Status::Auth(msg) => format!("需要授权码：{msg}"),
            Status::Failed(msg) => format!("连接失败：{msg}"),
        }
    }
    pub fn ok(&self) -> bool {
        matches!(self, Status::Ready)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub account: String,
    pub from: String,
    pub subject: String,
    /// 本地时间 `MM-DD HH:MM`
    pub time: String,
    pub code: Option<String>,
    /// 完整纯文字正文，保留分段，仅保存在内存中。
    pub summary: String,
    pub links: Vec<Link>,
    /// 会议邀请（来自 .ics / text/calendar），没有就是 `None`。
    pub meeting: Option<Meeting>,
    pub uid: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub label: String,
    pub url: String,
}

/// 仅把有效网页链接交给默认浏览器。
pub fn web_url(raw:&str)->Option<String> {
    let parsed=url::Url::parse(raw.trim()).ok()?;
    if !matches!(parsed.scheme(),"https"|"http") || parsed.host_str().is_none(){return None;}
    Some(parsed.into())
}

pub fn links_of(parsed:&Parsed)->Vec<Link> {
    use std::sync::OnceLock;
    static ANCHOR:OnceLock<regex::Regex>=OnceLock::new();
    static URL:OnceLock<regex::Regex>=OnceLock::new();
    let anchor=ANCHOR.get_or_init(||regex::Regex::new(r#"(?is)<a\b[^>]*?\s+href\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))[^>]*>(.*?)</a\s*>"#).unwrap());
    let url=URL.get_or_init(||regex::Regex::new(r#"(?i)https?://[^\s<>"'，。；！？、]+"#).unwrap());
    let mut links=Vec::new();let mut seen=std::collections::HashSet::new();
    let mut add=|raw:&str,label:String| {
        let decoded=otp::html_to_text(raw);
        if let Some(url)=web_url(decoded.trim()) {
            if seen.insert(url.clone()) {links.push(Link{label:if label.trim().is_empty(){url.clone()}else{label.trim().into()},url});}
        }
    };
    for cap in anchor.captures_iter(&parsed.html) {
        if let Some(href)=cap.get(1).or_else(||cap.get(2)).or_else(||cap.get(3)) {
            add(href.as_str(),otp::html_to_text(&cap[4]));
        }
    }
    for text in [&parsed.text,&otp::html_to_text(&parsed.html)] {
        for hit in url.find_iter(text) {
            let mut target=hit.as_str().trim_end_matches(['.',',',';',':','!','，','。','；','！','？','）','】','》']);
            for (open,close) in [('(',')'),('[',']'),('{','}')] {
                while target.ends_with(close) && target.chars().filter(|c|*c==close).count()>target.chars().filter(|c|*c==open).count(){target=&target[..target.len()-1];}
            }
            add(target,target.into());
        }
    }
    links
}

/// 会议邀请里界面和提醒要用的字段。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Meeting {
    pub all_day: bool,
    pub title: String,
    /// 开始时间（Unix 秒，本地时区解释）。
    pub start: i64,
    /// 来源提供的结束时间；缺失或无效时不推测时长。
    pub end: Option<i64>,
    /// 本地时间 `MM-DD HH:MM`，直接显示。
    pub at: String,
    /// iCalendar 的 `UID`；邮件和日历两条来源靠它 + 开始时间去重。
    pub uid: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Status { account: String, status: Status },
    Mail { item: Item, new: bool },
    Test { account: String, ok: bool, note: String },
}

// ---------------------------------------------------------------- 正文解析

pub struct Parsed {
    pub from: String,
    pub subject: String,
    /// text/plain 正文（没有则空）。
    pub text: String,
    /// text/html 正文转成的纯文本（没有则空）。
    pub html: String,
    /// text/calendar 正文（.ics 原文，没有则空）。
    pub calendar: String,
    pub emphasized: Vec<String>,
}

fn header(mail: &mailparse::ParsedMail, name: &str) -> String {
    mail.headers
        .iter()
        .find(|h| h.get_key().eq_ignore_ascii_case(name))
        .map(|h| h.get_value().trim().to_string())
        .unwrap_or_default()
}

fn display_addr(raw: &str) -> String {
    mailparse::addrparse(raw)
        .ok()
        .and_then(|list| {
            list.iter().find_map(|a| match a {
                mailparse::MailAddr::Single(s) => Some(s.display_name.clone().unwrap_or_else(|| s.addr.clone())),
                mailparse::MailAddr::Group(g) => g.addrs.first().map(|s| s.display_name.clone().unwrap_or_else(|| s.addr.clone())),
            })
        })
        .unwrap_or_else(|| raw.trim().trim_matches('"').to_string())
}

fn collect(mail: &mailparse::ParsedMail, plain: &mut String, html: &mut String, calendar: &mut String) {
    if mail.subparts.is_empty() {
        let ct = mail.ctype.mimetype.to_ascii_lowercase();
        if ct == "text/plain" {
            if let Ok(body) = mail.get_body() {
                plain.push_str(&body);
                plain.push('\n');
            }
        } else if ct == "text/html" {
            if let Ok(body) = mail.get_body() {
                html.push_str(&body);
                html.push('\n');
            }
        } else if ct == "text/calendar" || ct == "application/ics" {
            if let Ok(body) = mail.get_body() {
                calendar.push_str(&body);
                calendar.push('\n');
            }
        }
    } else {
        for part in &mail.subparts {
            collect(part, plain, html, calendar);
        }
    }
}

/// 解析一封原始邮件（头部 + 正文），只保留界面上要用的字段。
pub fn parse_message(raw: &[u8]) -> Parsed {
    let Ok(mail) = mailparse::parse_mail(raw) else {
        return Parsed {
            from: String::new(),
            subject: String::new(),
            text: String::from_utf8_lossy(raw).into_owned(),
            html: String::new(),
            calendar: String::new(),
            emphasized: Vec::new(),
        };
    };
    let subject = header(&mail, "Subject");
    let from = display_addr(&header(&mail, "From"));
    let (mut plain, mut html, mut calendar) = (String::new(), String::new(), String::new());
    collect(&mail, &mut plain, &mut html, &mut calendar);
    let emphasized = if plain.trim().is_empty() {
        otp::emphasized(&html)
    } else {
        Vec::new()
    };
    Parsed { from, subject, text: plain, html, calendar, emphasized }
}

/// 优先纯文本，HTML 邮件转成文字；保留完整正文与段落。
pub fn body_of(parsed:&Parsed)->String {
    let body = if !parsed.text.trim().is_empty() {
        parsed.text.clone()
    } else if !parsed.html.trim().is_empty() {
        otp::html_to_text(&parsed.html)
    } else {
        String::new()
    };
    body.replace("\r\n","\n").trim().to_string()
}

/// 紧凑场景使用的短摘要。
pub fn summary_of(parsed: &Parsed) -> String {
    let body=body_of(parsed);
    let flat = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: String = flat.chars().take(200).collect();
    if flat.chars().count() > 200 {
        out.push('…');
    }
    out
}

/// 从解析结果里提验证码：先纯文本，纯文本没有再看 HTML 文本。
pub fn code_of(parsed: &Parsed) -> Option<String> {
    if let Some(c) = otp::extract(&parsed.subject, &parsed.text, &parsed.emphasized) {
        return Some(c.text);
    }
    if !parsed.html.trim().is_empty() {
        let text = otp::html_to_text(&parsed.html);
        if let Some(c) = otp::extract(&parsed.subject, &text, &parsed.emphasized) {
            return Some(c.text);
        }
    }
    None
}

/// 从 .ics 文本里取第一个 VEVENT 的标题与开始时间（邮件正文用）。
pub fn meeting_of(calendar: &str) -> Option<Meeting> {
    meetings_of(calendar).into_iter().next()
}

/// 从一段 iCalendar 里取出所有带时刻的 VEVENT。
///
/// CalDAV 的 `calendar-query` 一次会返回多个事件，邮件里的 `.ics` 通常只有一个；
/// 两者共用这一套解析；支持带时刻和全天日期，没有有效 DTSTART 就跳过。
pub fn meetings_of(calendar: &str) -> Vec<Meeting> {
    // RFC 5545 折行：以空格/制表符开头的行是上一行的续行。
    let mut lines: Vec<String> = Vec::new();
    for raw in calendar.lines() {
        let line = raw.trim_end_matches('\r');
        if (line.starts_with(' ') || line.starts_with('\t')) && !lines.is_empty() {
            if let Some(last) = lines.last_mut() {
                last.push_str(&line[1..]);
            }
        } else {
            lines.push(line.to_string());
        }
    }

    let mut out = Vec::new();
    let mut fields: Vec<(String, String)> = Vec::new();
    let mut in_event = false;
    let mut cancelled = false;
    let mut nested = 0usize;
    for line in &lines {
        let up = line.to_ascii_uppercase();
        if up=="BEGIN:VCALENDAR" {cancelled=false;}
        if !in_event && up=="METHOD:CANCEL" {cancelled=true;}
        if up=="BEGIN:VEVENT" {
            in_event = true;
            nested=0;
            fields.clear();
            continue;
        }
        if up=="END:VEVENT" {
            if in_event && !cancelled {
                if let Some(m) = event_of(&fields) {
                    out.push(m);
                }
            }
            in_event = false;
            continue;
        }
        if !in_event {
            continue;
        }
        if up.starts_with("BEGIN:"){nested+=1;continue;}
        if up.starts_with("END:"){nested=nested.saturating_sub(1);continue;}
        if nested>0 {continue;}
        let Some((head, value)) = line.split_once(':') else { continue };
        let name = head.split(';').next().unwrap_or("").to_ascii_uppercase();
        if name == "DTSTART" || name == "DTEND" {
            if let Some(tz) = head.split(';').skip(1).find_map(|p| p.split_once('=').filter(|(k, _)| k.eq_ignore_ascii_case("TZID")).map(|(_, v)| v.trim_matches('"'))) {
                fields.push((format!("{name}-TZID"), tz.to_string()));
            }
        }
        fields.push((name, value.trim().to_string()));
    }
    out
}

fn event_of(fields: &[(String, String)]) -> Option<Meeting> {
    use chrono::TimeZone;
    let get = |name: &str| fields.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone());
    if get("STATUS").is_some_and(|v|v.eq_ignore_ascii_case("CANCELLED")){return None;}
    let parse_time = |name: &str| -> Option<chrono::DateTime<chrono::Local>> {
        let value = get(name)?;
        Some(if let Some(s) = value.strip_suffix('Z') {
            chrono::NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%S").ok()?.and_utc().with_timezone(&chrono::Local)
        } else {
            if !value.contains('T') {
                return chrono::NaiveDate::parse_from_str(&value,"%Y%m%d").ok()?.and_hms_opt(0,0,0)?.and_local_timezone(chrono::Local).earliest();
            }
            let n = chrono::NaiveDateTime::parse_from_str(&value, "%Y%m%dT%H%M%S").ok()?;
            if let Some(tz) = get(&format!("{name}-TZID")) {
                let zone: chrono_tz::Tz = tz.parse().ok()?;
                zone.from_local_datetime(&n).single()?.with_timezone(&chrono::Local)
            } else {
                chrono::Local.from_local_datetime(&n).single()?
            }
        })
    };
    let naive = parse_time("DTSTART")?;
    let all_day=get("DTSTART").is_some_and(|value|!value.contains('T'));
    let end = parse_time("DTEND").map(|at|at.timestamp()).filter(|end|*end>naive.timestamp())
        .or_else(||all_day.then(||naive.date_naive().succ_opt()?.and_hms_opt(0,0,0)?.and_local_timezone(chrono::Local).earliest().map(|at|at.timestamp())).flatten());
    let title = get("SUMMARY").map(|v| ics_text(&v)).filter(|t| !t.is_empty()).unwrap_or_else(|| "会议".to_string());
    Some(Meeting {
        all_day,
        title,
        start: naive.timestamp(),
        end,
        at: naive.format(if all_day {"%m-%d 全天"}else{"%m-%d %H:%M"}).to_string(),
        uid: get("UID").unwrap_or_default(),
    })
}

/// iCalendar 文本转义：`\n` / `\,` / `\;` / `\\`。
fn ics_text(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut it = v.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') | Some('N') => out.push(' '),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out.trim().to_string()
}

use crate::otp;

#[cfg(test)]
mod tests {
    #[test]
    fn quality_cancelled_meetings_are_not_active_events() {
        let event="BEGIN:VEVENT\nUID:cancelled\nDTSTART:20261101T060000Z\nSTATUS:CANCELLED\nEND:VEVENT";
        assert!(super::meetings_of(event).is_empty());
        let message=format!("BEGIN:VCALENDAR\nMETHOD:CANCEL\n{}\nEND:VCALENDAR",event.replace("STATUS:CANCELLED\n",""));
        assert!(super::meetings_of(&message).is_empty());
    }

    #[test]
    fn quality_alarm_fields_do_not_replace_meeting_fields() {
        let event="BEGIN:VEVENT\nDTSTART:20261101T060000Z\nBEGIN:VALARM\nSUMMARY:闹钟文案\nEND:VALARM\nSUMMARY:正式会议\nEND:VEVENT";
        assert_eq!(super::meetings_of(event)[0].title,"正式会议");
    }

    #[test]
    fn quality_ics_unfolding_preserves_content_spaces() {
        let event="BEGIN:VEVENT\nDTSTART:20261101T060000Z\nSUMMARY:项目\n  评审\nEND:VEVENT";
        assert_eq!(super::meetings_of(event)[0].title,"项目 评审");
    }
    #[test]
    fn email_links_preserve_targets_labels_and_query_parameters() {
        let parsed=super::parse_message(b"Content-Type: text/html; charset=utf-8\r\n\r\n<a href=\"https://example.com/review?a=1&amp;b=2\"><b>Review</b></a><a href='https://example.com/review?a=1&amp;b=2'>Again</a><a href=javascript:alert(1)>Unsafe</a>");
        assert_eq!(super::links_of(&parsed),vec![super::Link{label:"Review".into(),url:"https://example.com/review?a=1&b=2".into()}]);
        let plain=super::parse_message(b"Content-Type: text/plain\r\n\r\nRead https://example.com/docs. Then https://example.com/help");
        let links=super::links_of(&plain);
        assert_eq!(links.len(),2);assert_eq!(links[0].url,"https://example.com/docs");
        assert!(super::web_url("file:///C:/Windows/notepad.exe").is_none());
        assert!(super::web_url("https://").is_none());
        let parens=super::parse_message(b"Content-Type: text/plain\r\n\r\nhttps://example.com/Function_(mathematics). (https://example.com/help)");
        let targets=super::links_of(&parens);
        assert_eq!(targets[0].url,"https://example.com/Function_(mathematics)");
        assert_eq!(targets[1].url,"https://example.com/help");
    }
    use super::*;
    use chrono::TimeZone;

    fn raw(subject: &str, body: &str) -> Vec<u8> {
        format!("From: =?UTF-8?B?5bCP54mb?= <hi@example.com>\r\nSubject: {subject}\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{body}").into_bytes()
    }

    #[test]
    fn parses_subject_sender_and_code() {
        let parsed = parse_message(&raw("验证码", "您的验证码是：834921，10分钟内有效。"));
        assert_eq!(parsed.subject, "验证码");
        assert_eq!(parsed.from, "小牛");
        assert_eq!(code_of(&parsed).as_deref(), Some("834921"));
    }

    #[test]
    fn html_only_mail_still_finds_code() {
        let html = "<html><body><p>您正在登录</p><p>验证码：<b>906213</b></p></body></html>";
        let raw = format!("From: a@b.c\r\nSubject: 登录\r\nContent-Type: text/html; charset=utf-8\r\n\r\n{html}");
        let parsed = parse_message(raw.as_bytes());
        assert_eq!(code_of(&parsed).as_deref(), Some("906213"));
    }

    #[test]
    fn accounts_roundtrip_without_secrets() {
        let items = vec![
            Account { email: "a@qq.com".into(), provider: Provider::Qq, enabled: true },
            Account { email: "b@163.com".into(), provider: Provider::Mail163, enabled: false },
        ];
        let bytes = encode_accounts(&items);
        assert_eq!(decode_accounts(&bytes), items);
        let text = String::from_utf8(bytes).unwrap();
        assert!(!text.contains("password") && !text.contains("auth"));
        assert_eq!(decode_accounts(b"not json"), Vec::new());
    }

    #[test]
    fn provider_presets_cover_six_kinds() {
        assert_eq!(Provider::ALL.len(), 6);
        assert_eq!(Provider::parse("gmail"), Provider::Gmail);
        assert_eq!(Provider::Gmail.host(), "imap.gmail.com");
        assert_eq!(Provider::Gmail.port(), 993);
        let gmail=Account{email:"demo@gmail.com".into(),provider:Provider::Gmail,enabled:true};
        assert_eq!(decode_accounts(&encode_accounts(&[gmail.clone()])),vec![gmail]);
        assert!(Provider::Mail163.needs_id() && Provider::Mail126.needs_id());
        assert!(!Provider::Qq.needs_id());
        assert_eq!(Provider::parse("exmail").host(), "imap.exmail.qq.com");
    }

    #[test]
    fn summary_flattens_body_and_caps_length() {
        let parsed = parse_message(&raw("周会", "第一行\n\n  第二行   有空格\r\n第三行"));
        assert_eq!(summary_of(&parsed), "第一行 第二行 有空格 第三行");
        let long = "字".repeat(400);
        let parsed = parse_message(&raw("长", &long));
        let s = summary_of(&parsed);
        assert_eq!(s.chars().count(), 201, "200 字 + 省略号");
        assert!(s.ends_with('…'));
        // 没有正文时摘要为空，界面只显示主题
        assert_eq!(summary_of(&parse_message(&raw("空", ""))), "");
    }

    #[test]
    fn full_body_keeps_paragraphs_and_content_beyond_preview_and_old_fetch_limit() {
        let text=format!("第一段\n\n{}\n\n最后一段：完整正文结束", "正文内容".repeat(10000));
        let parsed=parse_message(&raw("长邮件",&text));
        assert_eq!(body_of(&parsed),text);
        let html=parse_message(b"Content-Type: text/html; charset=utf-8\r\n\r\n<p>First paragraph</p><p>Last paragraph</p>");
        assert_eq!(body_of(&html),"First paragraph\nLast paragraph");
    }

    const ICS: &str = "BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
BEGIN:VEVENT\r\n\
UID:demo-1@example.com\r\n\
SUMMARY:产品评审会\r\n\
DTSTART;TZID=Asia/Shanghai:20261030T143000\r\n\
DTEND;TZID=Asia/Shanghai:20261030T153000\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";

    #[test]
    fn ics_meeting_title_and_start() {
        let m = meeting_of(ICS).expect("应当识别出会议");
        assert_eq!(m.title, "产品评审会");
        let start = chrono::Utc
            .with_ymd_and_hms(2026, 10, 30, 6, 30, 0)
            .earliest()
            .unwrap();
        assert_eq!(m.at, start.with_timezone(&chrono::Local).format("%m-%d %H:%M").to_string());
        let expect = start.timestamp();
        assert_eq!(m.start, expect);
        assert_eq!(m.end,Some(expect+3600));
    }

    #[test]
    fn ics_end_time_keeps_its_timezone_and_never_invents_a_duration() {
        let ics="BEGIN:VEVENT\nDTSTART;TZID=Asia/Shanghai:20261030T143000\nDTEND:20261030T073000Z\nEND:VEVENT";
        let meeting=meeting_of(ics).unwrap();
        assert_eq!(meeting.end,Some(meeting.start+3600));
        let ics="BEGIN:VEVENT\nDTSTART:20261030T063000Z\nDTEND;TZID=Asia/Shanghai:20261030T153000\nEND:VEVENT";
        let meeting=meeting_of(ics).unwrap();
        assert_eq!(meeting.end,Some(meeting.start+3600));
        for end in ["","DTEND:20261030T060000Z\n","DTEND:invalid\n","DTEND;TZID=Unknown/Zone:20261030T153000\n"] {
            let meeting=meeting_of(&format!("BEGIN:VEVENT\nDTSTART:20261030T063000Z\n{end}END:VEVENT")).unwrap();
            assert!(meeting.end.is_none(),"缺失或无效的结束时间应保留为空");
        }
    }

    #[test]
    fn ics_utc_and_folded_lines() {
        // 折行：CRLF + 一个空格被删掉，"Design s" + "ync" = "Design sync"
        let utc = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nSUMMARY:Design s\n ync\nDTSTART:20261030T063000Z\nEND:VEVENT\nEND:VCALENDAR\n";
        let m = meeting_of(utc).expect("Z 时间应能解析");
        assert_eq!(m.title, "Design sync");
        let local = chrono::DateTime::from_timestamp(m.start, 0)
            .unwrap()
            .with_timezone(&chrono::Local)
            .format("%m-%d %H:%M")
            .to_string();
        assert_eq!(m.at, local);
    }

    #[test]
    fn ics_named_time_zone_uses_its_own_offset() {
        let ics = "BEGIN:VEVENT\nSUMMARY:跨时区会议\nDTSTART;TZID=America/New_York:20261030T090000\nEND:VEVENT\n";
        let m = meeting_of(ics).unwrap();
        assert_eq!(m.start, chrono::DateTime::parse_from_rfc3339("2026-10-30T13:00:00Z").unwrap().timestamp());
        let unknown = "BEGIN:VEVENT\nDTSTART;TZID=Unknown/Zone:20261030T090000\nEND:VEVENT\n";
        assert!(meeting_of(unknown).is_none());
    }

    #[test]
    fn all_day_calendar_event_has_exclusive_end() {
        let event=meeting_of("BEGIN:VEVENT\nUID:all-day\nSUMMARY:全天\nDTSTART;VALUE=DATE:20261103\nDTEND;VALUE=DATE:20261105\nEND:VEVENT").expect("全天日程应可展示");
        assert_eq!(event.end.unwrap()-event.start,2*86400);
    }

    #[test]
    fn no_ics_means_no_meeting() {
        // 普通邮件没有日历部分，不从正文猜测日期。
        let all_day = "BEGIN:VEVENT\nSUMMARY:放假\nDTSTART;VALUE=DATE:20261030\nEND:VEVENT\n";
        assert!(meeting_of(all_day).unwrap().all_day);
        assert!(meeting_of("").is_none());
        assert!(meeting_of("会议时间：10月30日 14:30").is_none());
    }

    #[test]
    fn calendar_part_is_collected_from_mime() {
        let raw = format!(
            "From: a@b.c\r\nSubject: 邀请\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=XYZ\r\n\r\n--XYZ\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n会议邀请见附件\r\n--XYZ\r\nContent-Type: text/calendar; charset=utf-8; method=REQUEST\r\n\r\n{ICS}\r\n--XYZ--\r\n"
        );
        let parsed = parse_message(raw.as_bytes());
        assert!(parsed.calendar.contains("产品评审会"));
        let m = meeting_of(&parsed.calendar).unwrap();
        assert_eq!(m.title, "产品评审会");
        assert_eq!(summary_of(&parsed), "会议邀请见附件");
    }
}
