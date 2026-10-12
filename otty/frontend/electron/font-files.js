// The licensed faces of docs/TYPOGRAPHY.md, recognised by file name. Styrene and
// Tiempos are commercial, so the repository ships none: whoever holds a licence
// drops the files into resources/fonts/licensed/ and they travel with the app.
const FAMILIES = [['styreneb', 'Styrene B'], ['styrenea', 'Styrene A'], ['tiempostext', 'Tiempos Text']];
// Longest names first: "semibold" contains "bold".
const WEIGHTS = [['semibold', 600], ['thin', 100], ['light', 300], ['medium', 500], ['bold', 700], ['black', 900]];

// `StyreneB-MediumItalic.otf` → { family: 'Styrene B', weight: 500, style: 'italic' }.
function describeFont(file) {
  if (!/\.(otf|ttf|woff2?)$/i.test(file)) return null;
  const name = file.toLowerCase().replace(/\.[^.]+$/, '').replace(/[^a-z0-9]/g, '');
  const family = FAMILIES.find(([prefix]) => name.startsWith(prefix));
  if (!family) return null;
  const rest = name.slice(family[0].length);
  return {
    file,
    family: family[1],
    weight: (WEIGHTS.find(([word]) => rest.includes(word)) || [, 400])[1],
    style: rest.includes('italic') ? 'italic' : 'normal',
  };
}

module.exports = { describeFont };
