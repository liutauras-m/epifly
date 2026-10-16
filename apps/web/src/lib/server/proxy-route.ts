/** Request handlers shared by the proxied route trees (`/v1`, `/api`, `/ui`, `/mcp`, `/admin`). */
import type { RequestHandler } from '@sveltejs/kit';
import { proxyToGateway } from './proxy.js';

const handler: RequestHandler = (event) => proxyToGateway(event);

export const GET = handler;
export const POST = handler;
export const PUT = handler;
export const PATCH = handler;
export const DELETE = handler;
export const OPTIONS = handler;
