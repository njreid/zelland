import { LoroDoc, LoroList, LoroMap } from 'loro-crdt';

export interface Comment {
	author: string;
	timestamp: string;
	body: string;
}

export interface Ann {
	id: string;
	thread: Comment[];
}

export function createAnnotationManager() {
	let doc: LoroDoc | null = null;
	let ws: WebSocket | null = null;
	let annotations = $state<Ann[]>([]);
	let reconnectTimeout: number | null = null;
	let reconnectDelay = 1000;
	const MAX_RECONNECT_DELAY = 30000;

	function readAnnotations(loroDoc: LoroDoc): Ann[] {
		const map = loroDoc.getMap('annotations');
		const result: Ann[] = [];

		const keys = map.keys();
		for (const key of keys) {
			const threadList = map.get(key) as LoroList;
			const thread: Comment[] = [];
			
			for (let i = 0; i < threadList.length; i++) {
				const commentMap = threadList.get(i) as LoroMap;
				thread.push({
					author: commentMap.get('author') as string,
					timestamp: commentMap.get('timestamp') as string,
					body: commentMap.get('body') as string
				});
			}

			result.push({ id: key, thread });
		}

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
		doc.importUpdate(new Uint8Array(data));
		onDocUpdate();
	}

	function connect(hostAddress: string, filepath: string) {
		if (ws && ws.url.includes(encodeURIComponent(filepath))) return;
		disconnect();

		doc = new LoroDoc();
		doc.subscribe(() => {
			onDocUpdate();
			if (doc && ws && ws.readyState === WebSocket.OPEN) {
				const update = doc.exportUpdate();
				ws.send(update);
			}
		});

		function doConnect() {
			if (reconnectTimeout) {
				clearTimeout(reconnectTimeout);
				reconnectTimeout = null;
			}

			const url = `ws://${hostAddress}:8083/annotations/sync/${encodeURIComponent(filepath)}`;
			ws = new WebSocket(url);
			ws.binaryType = 'arraybuffer';

			ws.onopen = () => {
				console.log('Annotation sync connected');
				reconnectDelay = 1000;
				if (doc) {
					// Send full state on connect
					ws?.send(doc.exportUpdate());
				}
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
			doc = null;
		}
		annotations = [];
		reconnectDelay = 1000;
	}

	function addComment(annId: string, author: string, body: string) {
		if (!doc) return;
		const map = doc.getMap('annotations');
		let threadList = map.get(annId) as LoroList;
		if (!threadList) {
			threadList = map.setContainer(annId, new LoroList());
		}
		
		const commentMap = threadList.insertContainer(threadList.length, new LoroMap());
		commentMap.set('author', author);
		commentMap.set('timestamp', new Date().toISOString());
		commentMap.set('body', body);
		
		onDocUpdate();
	}

	function deleteAnnotation(annId: string) {
		if (!doc) return;
		const map = doc.getMap('annotations');
		map.delete(annId);
		onDocUpdate();
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
