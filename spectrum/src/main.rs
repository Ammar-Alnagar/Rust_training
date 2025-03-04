use axum::{
    Json, Router,
    extract::{State, WebSocketUpgrade},
    response::{Html, IntoResponse},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::RwLock;
use tower_http::services::ServeDir;
use uuid::Uuid;

// Session and state management structures
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct Detail {
    detail: String,
    identified: bool,
    id: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct Session {
    prompt: Option<String>,
    image: Option<String>,
    image_description: Option<String>,
    chat: Vec<(String, String)>,
    treatment_plan: String,
    topic_focus: String,
    key_details: Vec<String>,
    identified_details: Vec<String>,
    used_hints: Vec<String>,
    difficulty: String,
    age: String,
    autism_level: String,
}

#[derive(Clone, Debug)]
struct AppState {
    sessions: Arc<RwLock<HashMap<Uuid, Session>>>,
    active_sessions: Arc<RwLock<HashMap<String, Uuid>>>,
    clients: Arc<RwLock<HashMap<String, tokio::sync::mpsc::Sender<String>>>>,
    huggingface_token: String,
    google_api_key: String,
    http_client: Client,
}

// API structures for external service communication
#[derive(Debug, Serialize, Deserialize)]
struct HuggingFaceRequest {
    inputs: String,
    parameters: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GoogleRequest {
    contents: Vec<GoogleContent>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GoogleContent {
    parts: Vec<GooglePart>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GooglePart {
    text: Option<String>,
    inline_data: Option<GoogleInlineData>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GoogleInlineData {
    mime_type: String,
    data: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct FeedbackResponse {
    feedback: String,
    newly_identified_details: Vec<String>,
    hint: String,
    score: f32,
    advance_difficulty: bool,
}

#[tokio::main]
async fn main() {
    // Load environment variables
    dotenv::dotenv().ok();
    let huggingface_token = std::env::var("HF_TOKEN").expect("HF_TOKEN must be set");
    let google_api_key = std::env::var("GOOGLE_API_KEY").expect("GOOGLE_API_KEY must be set");

    // Initialize state
    let state = AppState {
        sessions: Arc::new(RwLock::new(HashMap::new())),
        active_sessions: Arc::new(RwLock::new(HashMap::new())),
        clients: Arc::new(RwLock::new(HashMap::new())),
        huggingface_token,
        google_api_key,
        http_client: Client::new(),
    };

    // Set up routes
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/ws", get(ws_handler))
        .route("/generate_image", post(generate_image_handler))
        .route("/process_chat", post(process_chat_handler))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    // Start server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Server running on http://{}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

// Main page handler
async fn index_handler() -> impl IntoResponse {
    Html(include_str!("../templates/index.html"))
}

// WebSocket handler for real-time updates
async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: axum::extract::ws::WebSocket, state: AppState) {
    // WebSocket handling implementation for real-time UI updates
    // This would include logic for:
    // - Sending updates to checklist
    // - Updating chat messages
    // - Notifying of new images
}

// Generate image API endpoint
#[derive(Debug, Deserialize)]
struct GenerateImageRequest {
    age: String,
    autism_level: String,
    topic_focus: String,
    treatment_plan: String,
}

async fn generate_image_handler(
    State(state): State<AppState>,
    Json(request): Json<GenerateImageRequest>,
) -> impl IntoResponse {
    // 1. Generate prompt based on parameters
    let prompt = generate_prompt(
        "Very Simple",
        &request.age,
        &request.autism_level,
        &request.topic_focus,
        &request.treatment_plan,
        &state,
    )
    .await;

    // 2. Call Hugging Face API to generate image
    let image_data = generate_image(&prompt, &state).await;

    // 3. Use Gemini to generate image description
    let description = generate_description(
        &image_data,
        &prompt,
        "Very Simple",
        &request.topic_focus,
        &state,
    )
    .await;

    // 4. Extract key details from description
    let key_details = extract_key_details(&description, &state).await;

    // 5. Create new session
    let session_id = Uuid::new_v4();
    let session = Session {
        prompt: Some(prompt),
        image: Some(image_data.clone()),
        image_description: Some(description),
        difficulty: "Very Simple".to_string(),
        age: request.age,
        autism_level: request.autism_level,
        topic_focus: request.topic_focus,
        treatment_plan: request.treatment_plan,
        key_details,
        ..Default::default()
    };

    // 6. Store session
    state
        .sessions
        .write()
        .await
        .insert(session_id, session.clone());

    // 7. Create checklist from key details
    let checklist: Vec<Detail> = session
        .key_details
        .iter()
        .enumerate()
        .map(|(id, detail)| Detail {
            detail: detail.clone(),
            identified: false,
            id,
        })
        .collect();

    // 8. Return response with image and session data
    Json(json!({
        "image": image_data,
        "session_id": session_id.to_string(),
        "checklist": checklist
    }))
}

// Process chat API endpoint
#[derive(Debug, Deserialize)]
struct ProcessChatRequest {
    user_message: String,
    session_id: String,
}

async fn process_chat_handler(
    State(state): State<AppState>,
    Json(request): Json<ProcessChatRequest>,
) -> impl IntoResponse {
    let session_id = Uuid::parse_str(&request.session_id).unwrap();

    // 1. Get current session
    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id).unwrap();

    // 2. Evaluate the child's description
    let evaluation = compare_details(&request.user_message, session, &state).await;

    // 3. Parse evaluation response
    let (feedback, new_difficulty, should_advance, newly_identified) =
        parse_evaluation(&evaluation, session);

    // 4. Update session with identified details
    for detail in &newly_identified {
        if !session.identified_details.contains(detail) {
            session.identified_details.push(detail.clone());
        }
    }

    // 5. Add to chat history
    session
        .chat
        .push(("Child".to_string(), request.user_message));
    session.chat.push(("Teacher".to_string(), feedback.clone()));

    // 6. Check if all items are identified
    let all_identified = session.identified_details.len() >= session.key_details.len();

    // 7. Handle difficulty advancement or completion
    let mut new_image = None;
    if should_advance || all_identified {
        // Generate new image with updated difficulty
        let difficulty = if should_advance {
            new_difficulty
        } else {
            session.difficulty.clone()
        };
        let prompt = generate_prompt(
            &difficulty,
            &session.age,
            &session.autism_level,
            &session.topic_focus,
            &session.treatment_plan,
            &state,
        )
        .await;

        let image_data = generate_image(&prompt, &state).await;
        let description = generate_description(
            &image_data,
            &prompt,
            &difficulty,
            &session.topic_focus,
            &state,
        )
        .await;
        let key_details = extract_key_details(&description, &state).await;

        // Create new session
        session.prompt = Some(prompt);
        session.image = Some(image_data.clone());
        session.image_description = Some(description);
        session.difficulty = difficulty;
        session.key_details = key_details;
        session.identified_details = vec![];
        session.used_hints = vec![];
        session.chat = vec![];

        // Create advancement message
        let advancement_message = if should_advance {
            format!(
                "Congratulations! You've advanced to {} difficulty! Here's a new image to describe.",
                new_difficulty
            )
        } else {
            "Great job identifying all the details! Here's a new image at the same difficulty level.".to_string()
        };

        session
            .chat
            .push(("System".to_string(), advancement_message));

        // Create new checklist
        let checklist: Vec<Detail> = session
            .key_details
            .iter()
            .enumerate()
            .map(|(id, detail)| Detail {
                detail: detail.clone(),
                identified: false,
                id,
            })
            .collect();

        new_image = Some(image_data);

        // Return response with new image and updated session data
        return Json(json!({
            "chat": session.chat,
            "checklist": checklist,
            "new_image": new_image
        }));
    }

    // 8. Update checklist with newly identified items
    let checklist: Vec<Detail> = session
        .key_details
        .iter()
        .enumerate()
        .map(|(id, detail)| {
            let identified = session
                .identified_details
                .iter()
                .any(|identified| similar_details(identified, detail));
            Detail {
                detail: detail.clone(),
                identified,
                id,
            }
        })
        .collect();

    // 9. Return chat and updated checklist
    Json(json!({
        "chat": session.chat,
        "checklist": checklist,
        "new_image": null
    }))
}

// Helper functions for API integration
async fn generate_prompt(
    difficulty: &str,
    age: &str,
    autism_level: &str,
    topic_focus: &str,
    treatment_plan: &str,
    state: &AppState,
) -> String {
    // Format prompt query for Gemini
    let query = format!(
        r#"
        Follow the instructions below to generate an image generation prompt for an educational image intended for autistic children.
        Consider the following parameters:
          - Difficulty: {}
          - Age: {}
          - Autism Level: {}
          - Topic Focus: {}
          - Treatment Plan: {}
        Emphasize that the image should be clear, calming, and support understanding and communication. The style should match the difficulty level: for example, "Very Simple" produces very basic visuals while "Very Detailed" produces rich visuals.
        The image should specifically focus on the topic: "{}".
        Please generate a prompt that instructs the image generation engine to produce an image with:
        1. Clarity and simplicity (minimalist backgrounds, clear subject)
        2. Literal representation with defined borders and consistent style
        3. Soft, muted colors and reduced visual complexity
        4. Positive, calm scenes
        5. Clear focus on the specified topic
        Use descriptive and detailed language.
        "#,
        difficulty, age, autism_level, topic_focus, treatment_plan, topic_focus
    );

    // Call Google Gemini API
    let request = GoogleRequest {
        contents: vec![GoogleContent {
            parts: vec![GooglePart {
                text: Some(query),
                inline_data: None,
            }],
        }],
    };

    let response = state.http_client
        .post("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash-lite:generateContent")
        .query(&[("key", &state.google_api_key)])
        .json(&request)
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    // Extract prompt from response
    response["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("A simple, clear image of animals for autism education")
        .to_string()
}

async fn generate_image(prompt: &str, state: &AppState) -> String {
    // Call Hugging Face Inference API
    let request = HuggingFaceRequest {
        inputs: prompt.to_string(),
        parameters: {
            let mut map = HashMap::new();
            map.insert("guidance_scale".to_string(), json!(7.5));
            map.insert("negative_prompt".to_string(), json!("ugly, blurry, poorly drawn hands, lewd, nude, deformed, missing limbs, missing eyes, missing arms, missing legs"));
            map.insert("num_inference_steps".to_string(), json!(50));
            map
        },
    };

    let response = state.http_client
        .post("https://api-inference.huggingface.co/models/stabilityai/stable-diffusion-3.5-large-turbo")
        .header("Authorization", format!("Bearer {}", state.huggingface_token))
        .json(&request)
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();

    // Convert image bytes to base64
    let base64_image = general_purpose::STANDARD.encode(&response);
    format!("data:image/png;base64,{}", base64_image)
}

async fn generate_description(
    image_data_url: &str,
    prompt: &str,
    difficulty: &str,
    topic_focus: &str,
    state: &AppState,
) -> String {
    // Extract base64 image data
    let base64_img = image_data_url.split(',').nth(1).unwrap();

    // Format query for Gemini Vision
    let query = format!(
        r#"
        You are an expert educator specializing in teaching children with autism.
        Please provide a detailed description of this image that was generated based on the prompt:
        "{}"
        The image is intended for a child with autism, focusing on the topic: "{}" at a {} difficulty level.
        In your description:
        1. List all key objects, characters, and elements present in the image
        2. Describe colors, shapes, positions, and relationships between elements
        3. Note any emotions, actions, or interactions depicted
        4. Highlight details that would be important for the child to notice
        5. Organize your description in a structured, clear way
        Your description will be used as a reference to evaluate the child's observations,
        so please be comprehensive but focus on observable details rather than interpretations.
        "#,
        prompt, topic_focus, difficulty
    );

    // Call Google Gemini Vision API
    let request = GoogleRequest {
        contents: vec![GoogleContent {
            parts: vec![
                GooglePart {
                    text: None,
                    inline_data: Some(GoogleInlineData {
                        mime_type: "image/png".to_string(),
                        data: base64_img.to_string(),
                    }),
                },
                GooglePart {
                    text: Some(query),
                    inline_data: None,
                },
            ],
        }],
    };

    let response = state.http_client
        .post("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash-thinking-exp-01-21:generateContent")
        .query(&[("key", &state.google_api_key)])
        .json(&request)
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    // Extract description from response
    response["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("An image showing educational content")
        .to_string()
}

async fn extract_key_details(description: &str, state: &AppState) -> Vec<String> {
    // Format query to extract key details
    let query = format!(
        r#"
        From the following detailed image description, extract a list of 10-15 key details that a child might identify.
        Each detail should be a simple, clear phrase describing one observable element.
        Description:
        {}
        Format your response as a JSON array of strings, each representing one key detail.
        Example format: ["red ball on the grass", "smiling girl with brown hair", "blue sky with clouds"]
        "#,
        description
    );

    // Call Google Gemini API
    let request = GoogleRequest {
        contents: vec![GoogleContent {
            parts: vec![GooglePart {
                text: Some(query),
                inline_data: None,
            }],
        }],
    };

    let response = state.http_client
        .post("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash-lite:generateContent")
        .query(&[("key", &state.google_api_key)])
        .json(&request)
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    // Extract and parse JSON array from response
    let response_text = response["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("[]");

    // Find JSON array in text
    let re = regex::Regex::new(r"\[.*\]").unwrap();
    if let Some(json_match) = re.find(response_text) {
        let json_str = &response_text[json_match.start()..json_match.end()];
        if let Ok(details) = serde_json::from_str::<Vec<String>>(json_str) {
            return details;
        }
    }

    // Fallback default details
    vec![
        "object in image".to_string(),
        "color".to_string(),
        "shape".to_string(),
        "background".to_string(),
    ]
}

async fn compare_details(user_details: &str, session: &Session, state: &AppState) -> String {
    let image_description = session.image_description.as_ref().unwrap_or(&String::new());

    // Format chat history
    let mut history_text = String::new();
    if !session.chat.is_empty() {
        history_text.push_str("\n\n### Previous Conversation:\n");
        for (idx, (speaker, msg)) in session.chat.iter().enumerate() {
            history_text.push_str(&format!("Turn {}:\n{}: {}\n", idx + 1, speaker, msg));
        }
    }

    // Format key details and other context
    let key_details_text = format!(
        "\n\n### Key Details to Identify:\n{}",
        session
            .key_details
            .iter()
            .map(|d| format!("- {}", d))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let identified_details_text = if !session.identified_details.is_empty() {
        format!(
            "\n\n### Previously Identified Details:\n{}",
            session
                .identified_details
                .iter()
                .map(|d| format!("- {}", d))
                .collect::<Vec<_>>()
                .join("\n")
        )
    } else {
        String::new()
    };

    let used_hints_text = if !session.used_hints.is_empty() {
        format!(
            "\n\n### Previously Given Hints:\n{}",
            session
                .used_hints
                .iter()
                .map(|h| format!("- {}", h))
                .collect::<Vec<_>>()
                .join("\n")
        )
    } else {
        String::new()
    };

    // Create evaluation query
    let message_text = format!(
        r#"You are a kind and encouraging teacher helping a child with autism describe an image.

### Image Prompt:
{}

### Detailed Image Description (Reference):
{}

### Current Difficulty Level: {}
{}{}{}{}

### Child's Current Description:
'{}'

Evaluate the child's description compared to the key details list. Use simple, clear language.
Praise specific correct observations. If something important is missing, provide a gentle hint
that hasn't been given before.

Follow these guidelines:
1. DO NOT mention that you're evaluating or scoring the child.
2. Keep feedback warm, positive, and encouraging.
3. If giving a hint, make it specific but not too obvious.
4. Never repeat hints that have already been given.
5. Focus on details the child hasn't yet identified.
6. Acknowledge the child's progress.

Return your response as a JSON object with the following format:
{{
  "feedback": "Your encouraging response to the child",
  "newly_identified_details": ["list", "of", "new details", "the child identified"],
  "hint": "A new hint about something not yet identified",
  "score": <number from 0-100 based on how complete the description is>,
  "advance_difficulty": <boolean indicating if child should advance>
}}

Ensure the JSON is valid and contains all fields."#,
        session.prompt.as_ref().unwrap_or(&String::new()),
        image_description,
        session.difficulty,
        key_details_text,
        history_text,
        identified_details_text,
        used_hints_text,
        user_details
    );

    // Call Google Gemini API
    let request = GoogleRequest {
        contents: vec![GoogleContent {
            parts: vec![GooglePart {
                text: Some(message_text),
                inline_data: None,
            }],
        }],
    };

    let response = state.http_client
        .post("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash-thinking-exp-01-21:generateContent")
        .query(&[("key", &state.google_api_key)])
        .json(&request)
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    // Extract response
    response["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("{\"feedback\": \"Great effort! Keep describing what you see.\", \"newly_identified_details\": [], \"hint\": \"\", \"score\": 0, \"advance_difficulty\": false}")
        .to_string()
}

fn parse_evaluation(
    evaluation_text: &str,
    session: &mut Session,
) -> (String, String, bool, Vec<String>) {
    // Find and parse JSON
    let re = regex::Regex::new(r"\{.*\}").unwrap();
    if let Some(json_match) = re.find(evaluation_text) {
        let json_str = &evaluation_text[json_match.start()..json_match.end()];
        if let Ok(evaluation) = serde_json::from_str::<FeedbackResponse>(json_str) {
            // Extract evaluation data
            let feedback = evaluation.feedback;
            let newly_identified_details = evaluation.newly_identified_details;
            let hint = evaluation.hint;
            let advance_difficulty = evaluation.advance_difficulty;

            // Add hint to used hints
            if !hint.is_empty() && !session.used_hints.contains(&hint) {
                session.used_hints.push(hint.clone());
            }

            // Add hint to feedback if not already included
            let enhanced_feedback = if !hint.is_empty() && !feedback.contains(&hint) {
                format!("{}\n\n💡 Hint: {}", feedback, hint)
            } else {
                feedback
            };

            // Handle difficulty advancement
            let current_difficulty = &session.difficulty;
            let difficulties = vec![
                "Very Simple",
                "Simple",
                "Moderate",
                "Detailed",
                "Very Detailed",
            ];

            let mut new_difficulty = current_difficulty.clone();
            let should_advance = advance_difficulty;

            if advance_difficulty {
                if let Some(idx) = difficulties.iter().position(|&d| d == current_difficulty) {
                    if idx < difficulties.len() - 1 {
                        new_difficulty = difficulties[idx + 1].to_string();
                    }
                }
            }

            return (
                enhanced_feedback,
                new_difficulty,
                should_advance,
                newly_identified_details,
            );
        }
    }

    // Default return if parsing fails
    (
        "That's interesting! Can you tell me more about what you see?".to_string(),
        session.difficulty.clone(),
        false,
        vec![],
    )
}

fn similar_details(detail1: &str, detail2: &str) -> bool {
    // Simple similarity check - could be improved with NLP techniques
    detail1.to_lowercase().contains(&detail2.to_lowercase())
        || detail2.to_lowercase().contains(&detail1.to_lowercase())
        || detail1
            .split_whitespace()
            .any(|word| word.len() > 3 && detail2.to_lowercase().contains(&word.to_lowercase()))
}
