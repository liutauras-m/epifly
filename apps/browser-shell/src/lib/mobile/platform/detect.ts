export type Platform = 'ios' | 'android' | 'macos' | 'windows' | 'web';

export function detectPlatform(): Platform {
	const ua = navigator.userAgent;
	if (/iPhone|iPad|iPod/.test(ua)) return 'ios';
	if (/Android/.test(ua)) return 'android';
	if (/Win/.test(ua)) return 'windows';
	if (/Mac/.test(ua)) return 'macos';
	return 'web';
}

export function setPlatformTag(): void {
	document.documentElement.dataset.platform = detectPlatform();
}
