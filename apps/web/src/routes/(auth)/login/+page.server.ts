import { redirect } from "@sveltejs/kit";
import type { PageServerLoad } from "./$types";

// Allowed returnTo: same-origin paths only
const RETURN_TO_RE = /^\/(?!\/)[^?#]*(?:\?[^#]*)?(?:#.*)?$/;

function sanitizeReturnTo(raw: string | null): string {
  if (!raw) return "/";
  if (!RETURN_TO_RE.test(raw)) return "/";
  return raw;
}

export const load: PageServerLoad = async ({ locals, url }) => {
  // Already authenticated — redirect to returnTo
  if (locals.session) {
    const returnTo = sanitizeReturnTo(url.searchParams.get("returnTo"));
    throw redirect(302, returnTo);
  }
  return {};
};
