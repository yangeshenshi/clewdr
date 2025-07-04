use axum::{
    Json, RequestExt,
    extract::{FromRequest, Path, Request},
};

use crate::{
    config::CLEWDR_CONFIG,
    error::ClewdrError,
    gemini_body::GeminiQuery,
    gemini_state::{GeminiApiFormat, GeminiState},
    types::{claude_message::CreateMessageParams, gemini::request::GeminiRequestBody},
};

pub struct GeminiContext {
    pub model: String,
    pub vertex: bool,
    pub stream: bool,
    pub path: String,
    pub query: GeminiQuery,
    pub api_format: GeminiApiFormat,
}

pub struct GeminiPreprocess(pub GeminiRequestBody, pub GeminiContext);

impl FromRequest<GeminiState> for GeminiPreprocess {
    type Rejection = ClewdrError;

    async fn from_request(mut req: Request, state: &GeminiState) -> Result<Self, Self::Rejection> {
        let Path(path) = req.extract_parts::<Path<String>>().await?;
        let vertex = req.uri().to_string().contains("vertex");
        if vertex && !CLEWDR_CONFIG.load().vertex.validate() {
            return Err(ClewdrError::BadRequest(
                "Vertex is not configured".to_string(),
            ));
        }
        let mut model = path
            .split('/')
            .next_back()
            .map(|s| s.split_once(':').map(|s| s.0).unwrap_or(s).to_string());
        if vertex {
            model = CLEWDR_CONFIG.load().vertex.model_id.to_owned().or(model)
        }
        let Some(model) = model else {
            return Err(ClewdrError::BadRequest(
                "Model not found in path or vertex config".to_string(),
            ));
        };
        let query = req.extract_parts::<GeminiQuery>().await?;
        let ctx = GeminiContext {
            vertex,
            model,
            stream: path.contains("streamGenerateContent"),
            path,
            query,
            api_format: GeminiApiFormat::Gemini,
        };
        let Json(body) = Json::<GeminiRequestBody>::from_request(req, &()).await?;
        let mut state = state.clone();
        state.update_from_ctx(&ctx);
        if let Some(res) = state.try_from_cache(&body).await {
            return Err(ClewdrError::CacheFound(res));
        }
        Ok(GeminiPreprocess(body, ctx))
    }
}

pub struct GeminiOaiPreprocess(pub CreateMessageParams, pub GeminiContext);

impl FromRequest<GeminiState> for GeminiOaiPreprocess {
    type Rejection = ClewdrError;

    async fn from_request(req: Request, state: &GeminiState) -> Result<Self, Self::Rejection> {
        let vertex = req.uri().to_string().contains("vertex");
        if vertex && !CLEWDR_CONFIG.load().vertex.validate() {
            return Err(ClewdrError::BadRequest(
                "Vertex is not configured".to_string(),
            ));
        }
        let Json(body) = Json::<CreateMessageParams>::from_request(req, &()).await?;
        let model = body.model.to_owned();
        let stream = body.stream.unwrap_or_default();
        let ctx = GeminiContext {
            vertex,
            model,
            stream,
            path: String::new(),
            query: GeminiQuery::default(),
            api_format: GeminiApiFormat::OpenAI,
        };
        let mut state = state.clone();
        state.update_from_ctx(&ctx);
        if let Some(res) = state.try_from_cache(&body).await {
            return Err(ClewdrError::CacheFound(res));
        }
        Ok(GeminiOaiPreprocess(body, ctx))
    }
}
