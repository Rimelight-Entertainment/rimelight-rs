// Sketch of a Durable Object room using loro-crdt. Only the library API is used; for size measurement.
import { LoroDoc } from 'loro-crdt';

export default class RoomLoro {
  constructor(state) {
    this.state = state;
    this.doc = new LoroDoc();
    this.sockets = new Set();
    this.ready = state.blockConcurrencyWhile(async () => {
      const snap = await state.storage.get('snapshot');
      if (snap) this.doc.import(new Uint8Array(snap));
    });
  }

  async fetch(request) {
    await this.ready;
    const pair = new WebSocketPair();
    const [client, server] = Object.values(pair);
    server.accept();
    this.sockets.add(server);
    // Initial sync: send the full state as a snapshot.
    server.send(this.doc.export({ mode: 'snapshot' }));
    server.addEventListener('message', (ev) => this.onMessage(server, ev.data));
    server.addEventListener('close', () => this.sockets.delete(server));
    return new Response(null, { status: 101, webSocket: client });
  }

  async onMessage(from, data) {
    const update = new Uint8Array(data);
    this.doc.import(update);
    for (const ws of this.sockets) {
      if (ws !== from) ws.send(update);
    }
    await this.state.storage.put('snapshot', this.doc.export({ mode: 'snapshot' }));
  }
}
