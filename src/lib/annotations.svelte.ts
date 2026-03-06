// No loro-crdt import — daemon is the authoritative Loro state holder.
// The client communicates via JSON over WebSocket.

export interface Comment { author: string; timestamp: string; body: string; }
export interface Ann {
	id: string;
	quote: string;
	thread: Comment[];
	/** For code block annotations: "startLine:startChar->endLine:endChar" (0-indexed codepoints). */
	code_range?: string;
	/** For code block annotations: the handle used in `[comments](#handle)` anchor. */
	codeblock_handle?: string;
}

export function createAnnotationManager() {
	let ws: WebSocket | null = null;
	let annotations = $state<Ann[]>([]);
	let reconnectTimeout: number | null = null;
	let reconnectDelay = 1000;
	const MAX_RECONNECT_DELAY = 30000;

	function handleMessage(text: string) {
		try {
			const parsed = JSON.parse(text) as Ann[];
			annotations = parsed;
		} catch (e) {
			console.error('Annotation sync: failed to parse JSON message', e);
		}
	}

	function connect(hostAddress: string, filepath: string) {
		if (ws && ws.url.includes(encodeURIComponent(filepath))) return;
		disconnect();

		function doConnect() {
			if (reconnectTimeout) {
				clearTimeout(reconnectTimeout);
				reconnectTimeout = null;
			}

			const url = `ws://${hostAddress}:8083/annotations/sync/${encodeURIComponent(filepath)}`;
			ws = new WebSocket(url);

			ws.onopen = () => {
				console.log('Annotation sync connected');
				reconnectDelay = 1000;
			};

			ws.onmessage = (event) => {
				handleMessage(event.data as string);
			};

			ws.onerror = (event) => {
				console.error('Annotation sync error:', event);
			};

			ws.onclose = () => {
				console.log('Annotation sync disconnected');
				ws = null;
				// Only reconnect if we still want a connection (annotations state is intact)
				if (annotations !== null) {
					reconnectTimeout = window.setTimeout(() => {
						console.log(`Reconnecting annotation sync (delay ${reconnectDelay}ms)...`);
						reconnectDelay = Math.min(reconnectDelay * 2, MAX_RECONNECT_DELAY);
						doConnect();
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
		annotations = [];
		reconnectDelay = 1000;
	}

	function addComment(annId: string, author: string, body: string) {
		if (!ws || ws.readyState !== WebSocket.OPEN) {
			console.warn('Annotation sync: cannot send addComment — not connected');
			return;
		}
		ws.send(JSON.stringify({ type: 'add_comment', ann_id: annId, author, body }));
	}

	function deleteAnnotation(annId: string) {
		if (!ws || ws.readyState !== WebSocket.OPEN) {
			console.warn('Annotation sync: cannot send delete — not connected');
			return;
		}
		ws.send(JSON.stringify({ type: 'delete', ann_id: annId }));
	}

	return {
		get annotations() {
			return annotations;
		},
		connect,
		disconnect,
		addComment,
		deleteAnnotation
	};
}
