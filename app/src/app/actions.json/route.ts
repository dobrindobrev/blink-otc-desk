import { ACTIONS_CORS_HEADERS } from "@/lib/actions";

// Maps the human-facing offer pages to their Action API, so wallets and
// blink clients can unfurl a shared /offer/... link into a blink.
export function GET() {
  return Response.json(
    { rules: [{ pathPattern: "/offer/**", apiPath: "/api/actions/offer/**" }] },
    { headers: ACTIONS_CORS_HEADERS },
  );
}

export const OPTIONS = GET;
