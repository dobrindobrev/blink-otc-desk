// Minimal Solana Actions types and helpers, following the spec at
// https://github.com/solana-developers/solana-actions (v2.4).
import { BLOCKCHAIN_ID } from "./config";

export const ACTION_VERSION = "2.4";

export interface ActionParameter {
  name: string;
  label: string;
  type?: "number" | "text";
  required?: boolean;
  min?: number;
  max?: number;
}

export interface LinkedAction {
  type: "transaction";
  href: string;
  label: string;
  parameters?: ActionParameter[];
}

export interface ActionGetResponse {
  type: "action";
  icon: string;
  title: string;
  description: string;
  label: string;
  disabled?: boolean;
  error?: { message: string };
  links?: { actions: LinkedAction[] };
}

export interface ActionPostRequest {
  account: string;
}

export interface ActionPostResponse {
  type: "transaction";
  transaction: string;
  message?: string;
}

export interface ActionError {
  message: string;
}

export const ACTIONS_CORS_HEADERS: Record<string, string> = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "GET,POST,PUT,OPTIONS",
  "Access-Control-Allow-Headers": "Content-Type, Authorization, Content-Encoding, Accept-Encoding",
  "Access-Control-Expose-Headers": "X-Action-Version, X-Blockchain-Ids",
  "Content-Type": "application/json",
  "X-Action-Version": ACTION_VERSION,
  "X-Blockchain-Ids": BLOCKCHAIN_ID,
};

export function actionJson(body: ActionGetResponse | ActionPostResponse | ActionError | object, status = 200) {
  return Response.json(body, { status, headers: ACTIONS_CORS_HEADERS });
}

export function actionError(message: string, status = 400) {
  return actionJson({ message } satisfies ActionError, status);
}

export function preflight() {
  return new Response(null, { headers: ACTIONS_CORS_HEADERS });
}
