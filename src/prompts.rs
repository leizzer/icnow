use rmcp::model::{ErrorCode, ErrorData, GetPromptRequest, ServerResult};
use serde_json::json;

pub fn handle_list_prompts() -> Result<ServerResult, ErrorData> {
    let result = serde_json::from_value(json!({
        "prompts": [{
            "name": "create_memory",
            "description": "Instruct the agent to evaluate the context and save an architectural decision or project memory.",
            "arguments": [
                {
                    "name": "focus",
                    "description": "Optional focus area (e.g. 'auth flow', 'database schema')",
                    "required": false
                }
            ]
        }]
    })).unwrap();

    Ok(ServerResult::ListPromptsResult(result))
}

pub fn handle_get_prompt(req: &GetPromptRequest) -> Result<ServerResult, ErrorData> {
    if req.params.name == "create_memory" {
        let focus = req
            .params
            .arguments
            .as_ref()
            .and_then(|args| args.get("focus"))
            .and_then(|s| s.as_str())
            .unwrap_or("general architecture and patterns");

        let text = format!(
            "Please review our current conversation and the work we just completed, paying special attention to: **{focus}**.\n\nYour task is to extract highly relevant, persistent knowledge from our session and save it using the `save_memory` tool.\n\n**Guidelines for a good memory:**\n1. **Focus on the 'Why' and the 'How'**: Document architectural decisions, non-obvious design patterns, and gotchas. Importantly, explain how different parts of the system interact with each other. Do not just summarize what code was written.\n2. **Interconnect Nodes**: Use the `links` parameter to attach all relevant files, symbols, or components that are part of this interaction. The memory should act as a hub that connects these related nodes in the graph.\n3. **Keywords are critical**: Provide a rich set of relevant keywords (technologies, file names, conceptual terms) so this memory is easily discoverable via search.\n4. **Avoid Duplicates**: If you believe this memory might already exist, run `search_memories` first to verify, and only save if you are adding new context.\n\nPlease evaluate our context now and create the appropriate memory."
        );

        let result = serde_json::from_value(json!({
            "description": "Agent instruction to save a memory",
            "messages": [
                {
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": text
                    }
                }
            ]
        }))
        .unwrap();

        Ok(ServerResult::GetPromptResult(result))
    } else {
        Err(ErrorData {
            code: ErrorCode::INVALID_PARAMS,
            message: format!("Unknown prompt: {}", req.params.name).into(),
            data: None,
        })
    }
}
