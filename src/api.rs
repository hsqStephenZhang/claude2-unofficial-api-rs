use anyhow::Result;
use reqwest::{
    header::{self, ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE, COOKIE, HOST, REFERER, USER_AGENT},
    multipart, Body, Method, Proxy,
};
use serde_json::json;

use crate::{
    objects::{Conversation, History},
    utils::request,
};

pub struct Client {
    pub cookie: String,
    pub proxys: Vec<Proxy>,
    pub organization_id: String,
    base_header: header::HeaderMap,
}

impl Client {
    // create a new client, must provide cookie
    // if specific proxies, this method will check its availability(scheme)
    // supported proxies(by reqwest): http, https, socks5
    pub async fn try_new(cookie: &str, proxys: Vec<String>) -> Result<Self> {
        let mut headers: header::HeaderMap = header::HeaderMap::new();
        headers.insert(HOST, "claude.ai".parse()?);
        headers.insert(COOKIE, cookie.parse()?);
        headers.insert(REFERER, "https://claude.ai/chats".parse()?);
        headers.insert(CONTENT_TYPE, "application/json".parse()?);
        headers.insert(ACCEPT, "*/*".parse()?);
        headers.insert(USER_AGENT, "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/115.0.0.0 Safari/537.36 Edg/115.0.1901.183".parse()?);
        headers.insert(
            ACCEPT_LANGUAGE,
            "en-US,en;q=0.9,zh-CN;q=0.8,zh;q=0.7,en-GB;q=0.6".parse()?,
        );
        // headers.insert("sec-fetch-site", "same-origin".parse().unwrap());
        // headers.insert("sec-fetch-mode", "cors".parse().unwrap());
        // headers.insert("sec-fetch-dest", "empty".parse().unwrap());

        let mut reqwest_proxies = Vec::with_capacity(proxys.len());

        for proxy in &proxys {
            match Proxy::all(proxy) {
                Ok(p) => reqwest_proxies.push(p),
                Err(e) => {
                    println!("proxy {} is not supported, warning: {}", proxy, e);
                }
            }
        }

        let organization_id = get_organization_id(
            &request(
                Method::GET,
                "https://claude.ai/api/organizations",
                &reqwest_proxies,
                headers.clone(),
                None,
                None,
            )
            .await?,
        )?;

        Ok(Self {
            cookie: cookie.to_owned(),
            proxys: reqwest_proxies,
            organization_id: organization_id,
            base_header: headers,
        })
    }

    pub fn reset_proxy(&mut self) {
        self.proxys = vec![];
    }

    pub fn proxy(&mut self, proxy: &str) {
        match reqwest::Proxy::all(proxy) {
            Ok(p) => self.proxys.push(p),
            Err(e) => {
                println!("proxy {} is not supported, warning: {}", proxy, e);
            }
        }
    }
}

fn get_organization_id(content: &[u8]) -> Result<String> {
    let response: serde_json::Value = serde_json::from_slice(&content)?;
    response
        .as_array()
        .ok_or(anyhow::anyhow!("no organization info found"))?[0]
        .get("uuid")
        .map(|s| s.as_str().unwrap_or("").to_owned())
        .ok_or(anyhow::anyhow!("no organization id found"))
}

impl Client {
    // the handlers follow the same workflow:
    // 1. provide a url
    // 2. create the reqwest client and send the request
    // 3. check the response status code, parse data in the right way

    pub async fn list_all_conversations(&self) -> Result<Vec<Conversation>> {
        let conversations_url = format!(
            "https://claude.ai/api/organizations/{}/chat_conversations",
            self.organization_id
        );

        let content = request(
            Method::GET,
            &conversations_url,
            &self.proxys,
            self.base_header.clone(),
            None,
            None,
        )
        .await?;

        let result: Vec<Conversation> = serde_json::from_slice(&content)?;
        Ok(result)
    }

    pub const NEW_CHAT_NAME: &'static str = "test";

    // add
    pub async fn create_chat_conversation(&self) -> Result<String> {
        // format: 42ead3c7-4cb6-4599-a26f-e6e87b6d54db
        let id = uuid::Uuid::new_v4().to_string();

        request(
            Method::POST,
            &format!(
                "https://claude.ai/api/organizations/{}/chat_conversations",
                self.organization_id
            ),
            &self.proxys,
            self.base_header.clone(),
            Some(
                json!({
                    "uuid": id.clone(),
                    "name": Self::NEW_CHAT_NAME,
                })
                .to_string(),
            ),
            None,
        )
        .await
        .map(|_| id)
        .map_err(|_e| anyhow::anyhow!("create chat conversation failed"))
    }

    // delete
    pub async fn delete_chat_conversation(&self, conversation_id: &str) -> Result<()> {
        request(
            Method::DELETE,
            &format!(
                "https://claude.ai/api/organizations/{}/chat_conversations/{}",
                self.organization_id, conversation_id,
            ),
            &self.proxys,
            self.base_header.clone(),
            None,
            None,
        )
        .await
        .map(|_| Ok(()))
        .map_err(|_| anyhow::anyhow!("delete chat conversation failed"))?
    }

    // modify
    pub async fn rename_chat_conversation(
        &self,
        conversation_id: &str,
        new_title: &str,
    ) -> Result<()> {
        request(
            Method::POST,
            "https://claude.ai/api/rename_chat",
            &self.proxys,
            self.base_header.clone(),
            Some(
                json!(
                {
                    "organization_uuid": self.organization_id,
                    "conversation_uuid": conversation_id,
                    "title": new_title
                }
                )
                .to_string(),
            ),
            None,
        )
        .await
        .map(|_| Ok(()))
        .map_err(|_| anyhow::anyhow!("rename chat conversation failed"))?
    }

    // modify
    // upload attachment should accompany with a send_message request
    pub async fn upload_attachment(&self, filename: &str) -> Result<serde_json::Value> {
        let file = tokio::fs::File::open(filename).await?;

        // read file body stream
        let stream = tokio_util::codec::FramedRead::new(file, tokio_util::codec::BytesCodec::new());
        let file_body = Body::wrap_stream(stream);
        let some_file = multipart::Part::stream(file_body)
            .file_name("demo.pdf")
            .mime_str("application/pdf")?;

        let form = multipart::Form::new()
            .text("orgUuid", self.organization_id.clone())
            .part("file", some_file);

        let mut headers = self.base_header.clone();

        headers.insert(CONTENT_TYPE, "multipart/form-data".parse().unwrap());

        let content = request(
            Method::POST,
            "https://claude.ai/api/convert_document",
            &self.proxys,
            self.base_header.clone(),
            None,
            Some(form),
        )
        .await
        .map_err(|_| anyhow::anyhow!("rename file {} failed", filename))?;

        Ok(serde_json::from_slice(&content)?)
    }

    // modify
    // send message
    pub async fn send_message(
        &self,
        conversation_id: &str,
        prompt: &str,
        attachment: Option<&str>,
    ) -> Result<serde_json::Value> {
        let attachments = if let Some(attachment) = attachment {
            let attachment = self.upload_attachment(attachment).await?;
            vec![attachment.to_string()]
        } else {
            vec![]
        };
        request(
            Method::POST,
            "https://claude.ai/api/append_message",
            &self.proxys,
            self.base_header.clone(),
            Some(
                json!(
                {
                    "completion": {
                        "prompt": prompt,
                        "timezone": "Asia/Kolkata",
                        "model": "claude-2"
                    },
                    "organization_uuid": self.organization_id,
                    "conversation_uuid": conversation_id,
                    "text": prompt,
                    "attachments": attachments
                }
                )
                .to_string(),
            ),
            None,
        )
        .await
        .map(|bytes| {
            let s = std::str::from_utf8(&bytes).unwrap();
            let data = s
                .trim()
                .split("\n")
                .last()
                .ok_or(anyhow::anyhow!("wrong response format"))?;
            let data = &data[6..];
            let json = serde_json::from_str(data);

            Ok(json?)
        })
        .map_err(|_| anyhow::anyhow!("send message failed"))?
    }

    // query
    pub async fn chat_conversation_history(&self, conversation_id: &str) -> Result<History> {
        request(
            Method::GET,
            &format!(
                "https://claude.ai/api/organizations/{}/chat_conversations/{}",
                self.organization_id, conversation_id,
            ),
            &self.proxys,
            self.base_header.clone(),
            None,
            None,
        )
        .await
        .map(|bytes| Ok(serde_json::from_slice(&bytes)?))
        .map_err(|_| anyhow::anyhow!("rename chat conversation failed"))?
    }
}

#[cfg(test)]
mod tests {
    use super::Client;

    #[tokio::test]
    async fn test_full_workflow() -> anyhow::Result<()> {
        // std::env::set_var("RUST_LOG", "trace");
        // env_logger::init();

        let client = Client::try_new(
            todo!("please use your cookie instead"),
            vec!["http://127.0.0.1:8088".to_string()],
        )
        .await?;
        let mut conversations = client.list_all_conversations().await?;
        conversations.sort_by(|a: &crate::objects::Conversation, b| {
            a.created_at.timestamp().cmp(&b.created_at.timestamp())
        });
        conversations.sort_by(|a, b| a.updated_at.timestamp().cmp(&b.updated_at.timestamp()));

        conversations.iter().for_each(|c| {
            println!("uuid:{:?}, name:{:?}", c.uuid, c.name);
        });

        // create chat
        let new_conversation_id = client.create_chat_conversation().await?;

        // query chat history && name
        let his = client
            .chat_conversation_history(&new_conversation_id)
            .await?;

        // should be default by now
        assert!(his.name == Client::NEW_CHAT_NAME);
        assert!(his.chat_messages.len() == 0);

        let new_title = "new title xxx";

        // rename its name
        let _rename_result = client
            .rename_chat_conversation(&new_conversation_id, new_title)
            .await?;

        let answer = client
            .send_message(&new_conversation_id, "hello world", None)
            .await?;
        println!("response: {}", answer);

        // should have chat history now, name should be changed
        let his = client
            .chat_conversation_history(&new_conversation_id)
            .await?;

        for message in &his.chat_messages {
            println!("{}:{}", message.sender, message.text);
        }
        assert!(his.chat_messages.len() == 2);
        assert!(
            his.chat_messages
                .iter()
                .filter(|m| m.sender == "human")
                .count()
                == 1
        );
        assert!(
            his.chat_messages
                .iter()
                .filter(|m| m.sender == "assistant")
                .count()
                == 1
        );

        assert!(his.name == new_title);

        let _delete_result = client
            .delete_chat_conversation(&new_conversation_id)
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_upload_attachment_alone() -> anyhow::Result<()> {
        let filename: &str = "/Users/zc/Downloads/gdb_cheat_sheet2.pdf";
        let client = Client::try_new(
            todo!("please use your cookie instead"),
            vec!["http://127.0.0.1:8088".to_string()],
        )
        .await?;
        client.upload_attachment(filename).await?;

        Ok(())
    }
}
