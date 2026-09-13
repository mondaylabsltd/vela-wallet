// The extension's half of the "web page is the doorbell" channel.
//
// A page on our own origin connects a port and hands over a signing intent; we
// open the signing tab, and post the answer back down the same port. The port
// matters: a plain sendMessage callback dies with the service worker, while a
// connected port keeps it alive and, if it does die, the requester sees the
// disconnect instead of waiting forever.
const pending = new Map();

chrome.action.onClicked.addListener(() => {
  chrome.tabs.create({ url: chrome.runtime.getURL('index.html') });
});

chrome.runtime.onConnectExternal.addListener((port) => {
  if (port.name !== 'vela-sign') {
    port.disconnect();
    return;
  }
  port.onMessage.addListener((message) => {
    if (!message || message.vela !== 'intent') return;
    const id = crypto.randomUUID();
    pending.set(id, {
      intent: message.intent,
      context: message.context || {},
      // Filled in by the browser when the port was opened — the requester
      // cannot forge it. This is what lets the sheet skip the "self-reported
      // identity" warning on this channel.
      requester: port.sender && (port.sender.origin || port.sender.url),
      originVerified: true,
      port,
    });
    chrome.tabs.create({ url: chrome.runtime.getURL(`sign.html?ch=ext&id=${id}`) });
  });
  port.onDisconnect.addListener(() => {
    for (const [id, entry] of pending) {
      if (entry.port === port) pending.delete(id);
    }
  });
});

// Messages from our own signing page.
chrome.runtime.onMessage.addListener((message, sender, reply) => {
  if (!message || !message.vela) return false;
  const entry = pending.get(message.id);

  if (message.vela === 'take') {
    reply(entry ? {
      intent: entry.intent,
      context: entry.context,
      requester: entry.requester,
      originVerified: entry.originVerified,
    } : null);
    return true;
  }

  if (message.vela === 'result' || message.vela === 'error') {
    if (entry) {
      try {
        entry.port.postMessage(message.vela === 'result'
          ? { vela: 'result', result: message.result }
          : { vela: 'error', code: message.code });
      } catch (error) {
        // The requester closed its tab; nothing left to tell.
      }
      pending.delete(message.id);
    }
    reply({ ok: true });
    return true;
  }
  return false;
});
