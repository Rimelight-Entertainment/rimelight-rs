// Sketch of a Durable Object room using Yjs. Only the library API is used; for size measurement.
import * as Y from 'yjs';

export default class RoomYjs {
  constructor(state) {
    this.state = state;
    this.doc = new Y.Doc();
    this.sockets = new Set();
    this.ready = state.blockConcurrencyWhile(async () => {
      const snap = await state.storage.get('snapshot');
      if (snap) Y.applyUpdate(this.doc, new Uint8Array(snap));
    });
  }

  async fetch(request) {
    await this.ready;
    const pair = new WebSocketPair();
    const [client, server] = Object.values(pair);
    server.accept();
    this.sockets.add(server);
    // Initial sync: send the full state as one update.
    server.send(Y.encodeStateAsUpdate(this.doc));
    server.addEventListener('message', (ev) => this.onMessage(server, ev.data));
    server.addEventListener('close', () => this.sockets.delete(server));
    return new Response(null, { status: 101, webSocket: client });
  }

  async onMessage(from, data) {
    const update = new Uint8Array(data);
    Y.applyUpdate(this.doc, update, 'room');
    for (const ws of this.sockets) {
      if (ws !== from) ws.send(update);
    }
    await this.state.storage.put('snapshot', Y.encodeStateAsUpdate(this.doc));
  }
}
