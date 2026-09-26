// Generated from crates/ol-rht-chat/src/protocol.rs. Run `just chat-types`.
export type ChatRequest = {
/**
 * The selected branch, in chronological order; never the whole branch tree.
 */
messages: Array<ChatMessage>, };
export type ChatMessage = { id: string, role: Role, content: Array<ContentPart>, };
export type Role = "user" | "assistant";
export type ContentPart = { "type": "text", text: string, } | { "type": "reasoning", text: string, } | { "type": "tool_call", tool_call_id: string, call: ToolCall, result: ToolResult | null, };
export type ToolCall = { "name": "search", "args": SearchArgs } | { "name": "get_paper", "args": PaperArgs };
export type ToolResult = { "name": "search", "data": SearchResult } | { "name": "get_paper", "data": PaperResult };
export type PaperArgs = { id: string, };
export type PaperResult = { "status": "ok", paper: Paper, answered: CorpusProvenance | null, } | { "status": "error", message: string, };
export type Paper = { public_id: string, stub: boolean, title: string, authors: Array<string>, authors_truncated: boolean, venue: string, published: PaperDate | null, identifiers: Array<PaperIdentifier>, abstract_text: string, abstract_truncated: boolean, retracted_by: Array<string>, corrected_by: Array<string>, concern_by: Array<string>, open_access: boolean | null, full_text_url: string | null, publisher_url: string | null, };
export type PaperDate = { year: number, month: number, day: number, };
export type PaperIdentifier = { namespace: string, value: string, standing: string, };
export type SearchArgs = { query: string, limit: number, };
export type SearchResult = { "status": "ok", query: string, total: number, hits: Array<SearchPaper>, answered: CorpusProvenance | null, } | { "status": "error", message: string, };
export type SearchPaper = { public_id: string, title: string,
/**
 * At most six authors, in source order, to keep model context bounded.
 */
authors: Array<string>, venue: string, year: number | null, retracted_by: Array<string>, corrected_by: Array<string>, concern_by: Array<string>, };
export type CorpusProvenance = { epoch: number, batch: number, at: string, sources: Array<string>, attribution: Array<string>, };
export type PartKind = "text" | "reasoning";
export type ChatEvent = { "type": "start", message_id: string, } | { "type": "delta", part_id: string, kind: PartKind, text: string, } | { "type": "tool_call", part_id: string, call: ToolCall, } | { "type": "tool_result", part_id: string, result: ToolResult, } | { "type": "finish", reason: FinishReason, } | { "type": "error", message: string, };
export type FinishReason = "stop" | "length";
