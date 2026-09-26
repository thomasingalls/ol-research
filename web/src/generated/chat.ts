// Generated from crates/ol-rht-chat/src/protocol.rs. Run `just chat-types`.
export type ChatRequest = {
/**
 * The selected branch, in chronological order; never the whole branch tree.
 */
messages: Array<ChatMessage>, };
export type ChatMessage = { id: string, role: Role, content: Array<ContentPart>, };
export type Role = "user" | "assistant";
export type ContentPart = { "type": "text", text: string, } | { "type": "reasoning", text: string, };
export type PartKind = "text" | "reasoning";
export type ChatEvent = { "type": "start", message_id: string, } | { "type": "delta", part_id: string, kind: PartKind, text: string, } | { "type": "finish", reason: FinishReason, } | { "type": "error", message: string, };
export type FinishReason = "stop" | "length";
