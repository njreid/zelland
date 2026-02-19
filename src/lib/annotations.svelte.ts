/**
 * Annotation manager — YJS CRDT sync with the daemon.
 *
 * Connects via WebSocket to /annotations/sync/{filepath} on the daemon,
 * using the standard y-websocket binary protocol (lib0 varUint framing).
 *
 * Exposes a reactive `annotations` array (Svelte 5 runes) that rebuilds
 * from the YJS document on every change.
 */

import * as Y from 'yjs';
import * as encoding from 'lib0/encoding';
import * as decoding from 'lib0/decoding';

// y-websocket message types
const MSG_SYNC = 0;
// y-protocols sync message types
const SYNC_STEP1 = 0;
const SYNC_STEP2 = 1;
const SYNC_UPDATE = 2;

export interface Selector {
	quote: string;
	prefix: string;
	suffix: string;
}

export interface Comment {
	id: string;
	author: string;
	created: string;
	body: string;
}

export interface Ann {
	id: string;
	selector: Selector;
	thread: Comment[];
}

function randomId(len: number): string {
	const chars = 'abcdefghijklmnopqrstuvwxyz0123456789';
	let result = '';
	const arr = new Uint8Array(len);
	crypto.getRandomValues(arr);
	for (const b of arr) {
		result += chars[b % chars.length];
	}
	return result;
}

export function createAnnotationManager() {
	let doc: Y.Doc | null = null;
	let ws: WebSocket | null = null;
	let annotations = $state<Ann[]>([]);
	let reconnectTimeout: number | null = null;
	let reconnectDelay = 1000;
	const MAX_RECONNECT_DELAY = 30000;

	function readAnnotations(ydoc: Y.Doc): Ann[] {
		const map = ydoc.getMap('annotations');
		const result: Ann[] = [];

		map.forEach((value, key) => {
			if (!(value instanceof Y.Map)) return;
			const annMap = value as Y.Map<any>;

			const selMap = annMap.get('selector') as Y.Map<string> | undefined;
			const selector: Selector = selMap
				? {
						quote: selMap.get('quote') ?? '',
						prefix: selMap.get('prefix') ?? '',
						suffix: selMap.get('suffix') ?? ''
					}
				: { quote: '', prefix: '', suffix: '' };

			const threadArr = annMap.get('thread') as Y.Array<Y.Map<string>> | undefined;
			const thread: Comment[] = [];
			if (threadArr) {
				threadArr.forEach((item) => {
					if (item instanceof Y.Map) {
						thread.push({
							id: item.get('id') ?? '',
							author: item.get('author') ?? '',
							created: item.get('created') ?? '',
							body: item.get('body') ?? ''
						});
					}
				});
			}

			result.push({ id: key, selector, thread });
		});

		result.sort((a, b) => a.id.localeCompare(b.id));
		return result;
	}

	function onDocUpdate() {
		if (doc) {
			annotations = readAnnotations(doc);
		}
	}

	function handleMessage(data: ArrayBuffer) {
		if (!doc) return;
		const decoder = decoding.createDecoder(new Uint8Array(data));
		const msgType = decoding.readVarUint(decoder);

		if (msgType === MSG_SYNC) {
			const syncType = decoding.readVarUint(decoder);

			if (syncType === SYNC_STEP1) {
				// Server sent its state vector — reply with our diff (SyncStep2)
				const serverSv = decoding.readVarUint8Array(decoder);
				const update = Y.encodeStateAsUpdate(doc, serverSv);

				const encoder = encoding.createEncoder();
				encoding.writeVarUint(encoder, MSG_SYNC);
				encoding.writeVarUint(encoder, SYNC_STEP2);
				encoding.writeVarUint8Array(encoder, update);
				ws?.send(encoding.toUint8Array(encoder));

				// Also send our SyncStep1 so the server sends us what we're missing
				const sv = Y.encodeStateVector(doc);
				const encoder2 = encoding.createEncoder();
				encoding.writeVarUint(encoder2, MSG_SYNC);
				encoding.writeVarUint(encoder2, SYNC_STEP1);
				encoding.writeVarUint8Array(encoder2, sv);
				ws?.send(encoding.toUint8Array(encoder2));
			} else if (syncType === SYNC_STEP2 || syncType === SYNC_UPDATE) {
				const update = decoding.readVarUint8Array(decoder);
				Y.applyUpdate(doc, update);
			}
		}
		// MSG_AWARENESS: ignored for now
	}

	function connect(hostAddress: string, filepath: string) {
		if (ws && ws.url.includes(filepath)) return; // Already connected or connecting to this file
		disconnect();

		doc = new Y.Doc();

		// Observe deep changes on the annotations map
		const map = doc.getMap('annotations');
		map.observeDeep(onDocUpdate);

		// Also send local updates to the server
		doc.on('update', (update: Uint8Array, origin: any) => {
			if (origin === 'remote') return;
			if (!ws || ws.readyState !== WebSocket.OPEN) return;

			const encoder = encoding.createEncoder();
			encoding.writeVarUint(encoder, MSG_SYNC);
			encoding.writeVarUint(encoder, SYNC_UPDATE);
			encoding.writeVarUint8Array(encoder, update);
			ws.send(encoding.toUint8Array(encoder));
		});

		function doConnect() {
			if (reconnectTimeout) {
				clearTimeout(reconnectTimeout);
				reconnectTimeout = null;
			}

			const url = `ws://${hostAddress}:8083/annotations/sync/${filepath}`;
			ws = new WebSocket(url);
			ws.binaryType = 'arraybuffer';

			ws.onopen = () => {
				console.log('Annotation sync connected');
				reconnectDelay = 1000; // Reset delay on success
			};

			ws.onmessage = (event) => {
				handleMessage(event.data);
			};

			ws.onerror = (event) => {
				console.error('Annotation sync error:', event);
			};

			ws.onclose = () => {
				console.log('Annotation sync disconnected');
				ws = null;
				// Only reconnect if the doc still exists (meaning we haven't explicitly disconnected)
				if (doc) {
					reconnectTimeout = window.setTimeout(() => {
						console.log(`Reconnecting annotation sync (delay ${reconnectDelay}ms)...`);
						doConnect();
						reconnectDelay = Math.min(reconnectDelay * 2, MAX_RECONNECT_DELAY);
					}, reconnectDelay);
				}
			};
		}

		doConnect();
	}

	function disconnect() {
		if (reconnectTimeout) {
			clearTimeout(reconnectTimeout);
			reconnectTimeout = null;
		}
		if (ws) {
			ws.onclose = null;
			ws.onerror = null;
			ws.onmessage = null;
			ws.onopen = null;
			if (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING) {
				ws.close();
			}
			ws = null;
		}
		if (doc) {
			doc.destroy();
			doc = null;
		}
		annotations = [];
		reconnectDelay = 1000;
	}

	function createAnnotation(
		quote: string,
		prefix: string,
		suffix: string,
		author: string,
		body?: string
	): string | null {
		if (!doc) return null;

		const id = randomId(5);
		const commentId = 'c' + randomId(3);
		const now = new Date().toISOString();

		const map = doc.getMap('annotations');
		const annMap = new Y.Map<any>();
		const selector = new Y.Map<string>();
		selector.set('quote', quote);
		selector.set('prefix', prefix);
		selector.set('suffix', suffix);
		annMap.set('selector', selector);

		const thread = new Y.Array<Y.Map<string>>();
		const comment = new Y.Map<string>();
		comment.set('id', commentId);
		comment.set('author', author);
		comment.set('created', now);
		comment.set('body', body ?? '');
		thread.push([comment]);
		annMap.set('thread', thread);

		map.set(id, annMap);
		return id;
	}

	function addComment(annId: string, author: string, body: string): string | null {
		if (!doc) return null;

		const map = doc.getMap('annotations');
		const annMap = map.get(annId) as Y.Map<any> | undefined;
		if (!annMap) return null;

		const thread = annMap.get('thread') as Y.Array<Y.Map<string>> | undefined;
		if (!thread) return null;

		const commentId = 'c' + randomId(3);
		const comment = new Y.Map<string>();
		comment.set('id', commentId);
		comment.set('author', author);
		comment.set('created', new Date().toISOString());
		comment.set('body', body);
		thread.push([comment]);

		return commentId;
	}

	function deleteAnnotation(annId: string) {
		if (!doc) return;
		const map = doc.getMap('annotations');
		map.delete(annId);
	}

	return {
		get annotations() {
			return annotations;
		},
		connect,
		disconnect,
		createAnnotation,
		addComment,
		deleteAnnotation
	};
}
