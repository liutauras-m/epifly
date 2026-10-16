/**
 * AES-256-GCM sealing for cookie payloads.
 *
 * Wire format: base64url( iv[12] | ciphertext | tag[16] ). The purpose string is
 * bound as additional authenticated data, so a sealed OIDC transaction can never
 * be replayed as a session (or the other way round).
 */
import { createCipheriv, createDecipheriv, randomBytes } from 'node:crypto';

const IV_BYTES = 12;
const TAG_BYTES = 16;

export type SealPurpose = 'epifly.session.v1' | 'epifly.oidc-tx.v1';

export function seal(key: Buffer, purpose: SealPurpose, value: unknown): string {
	const iv = randomBytes(IV_BYTES);
	const cipher = createCipheriv('aes-256-gcm', key, iv);
	cipher.setAAD(Buffer.from(purpose, 'utf8'));
	const ct = Buffer.concat([cipher.update(JSON.stringify(value), 'utf8'), cipher.final()]);
	return Buffer.concat([iv, ct, cipher.getAuthTag()]).toString('base64url');
}

/** Returns the decoded value, or null when the input is malformed, tampered or sealed for another purpose. */
export function unseal<T>(key: Buffer, purpose: SealPurpose, sealed: string): T | null {
	const raw = Buffer.from(sealed, 'base64url');
	if (raw.length <= IV_BYTES + TAG_BYTES) return null;
	try {
		const decipher = createDecipheriv('aes-256-gcm', key, raw.subarray(0, IV_BYTES));
		decipher.setAAD(Buffer.from(purpose, 'utf8'));
		decipher.setAuthTag(raw.subarray(raw.length - TAG_BYTES));
		const plain = Buffer.concat([
			decipher.update(raw.subarray(IV_BYTES, raw.length - TAG_BYTES)),
			decipher.final(),
		]);
		return JSON.parse(plain.toString('utf8')) as T;
	} catch {
		return null;
	}
}
