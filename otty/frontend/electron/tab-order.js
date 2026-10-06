function moveTab(order, tabId, targetId, after = false) {
  if (tabId === targetId) return [...order];
  const sourceIndex = order.indexOf(tabId);
  if (sourceIndex < 0 || !order.includes(targetId)) return [...order];
  const next = order.filter((id) => id !== tabId);
  const targetIndex = next.indexOf(targetId);
  next.splice(targetIndex + Number(after), 0, tabId);
  return next;
}

function activeAfterClose(previousOrder, closedId, remainingOrder) {
  if (remainingOrder.length === 0) return null;
  const closedIndex = previousOrder.indexOf(closedId);
  return remainingOrder[Math.min(Math.max(closedIndex, 0), remainingOrder.length - 1)];
}

module.exports = { moveTab, activeAfterClose };
