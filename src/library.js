// Biblioteca de imágenes incluidas (src/img/apps/<id>.png).
// Iconos "Liquid Glass" de Icons8 (https://icons8.com), con atribución en el README y en los ajustes.
window.ORBE_LIBRARY = [
  { id: "discord", nombre: "Discord", match: ["discord"] },
  { id: "spotify", nombre: "Spotify", match: ["spotify"] },
  { id: "steam", nombre: "Steam", match: ["steam"] },
  { id: "whatsapp", nombre: "WhatsApp", match: ["whatsapp"] },
  { id: "github", nombre: "GitHub", match: ["github"] },
  { id: "claude", nombre: "Claude", match: ["claude"] },
  { id: "ai", nombre: "IA", match: ["chatgpt", "copilot", "gemini", "openai", "perplexity"] }
];

// "lib:<id>" si el nombre de la app coincide con alguna imagen de la biblioteca; si no, "".
window.ORBE_LIBRARY_MATCH = function (nombre) {
  const n = (nombre || "").toLowerCase();
  const hit = window.ORBE_LIBRARY.find(l => l.match.some(m => n.includes(m)));
  return hit ? "lib:" + hit.id : "";
};
