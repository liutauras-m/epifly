import { redirect } from "@sveltejs/kit";
import type { LayoutServerLoad } from "./$types";

export const load: LayoutServerLoad = async ({ locals, url }) => {
  if (!locals.session) {
    const returnTo = encodeURIComponent(url.pathname + url.search);
    throw redirect(302, `/auth/login?returnTo=${returnTo}`);
  }

  return {
    session: {
      userSub: locals.session.userSub,
      tenantOrgId: locals.session.tenantOrgId,
      displayName: locals.session.displayName,
      emailVerified: locals.session.emailVerified,
    },
  };
};
