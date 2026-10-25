/* mhtml Mermaid support: replaces each .mermaid-diagram source with an SVG
 * rendered by Ondine, which is embedded in the page. The source stays visible
 * when the renderer cannot be loaded or the diagram is invalid. */
(function () {
  var diagrams = document.querySelectorAll('.mermaid-diagram');
  if (!diagrams.length) return;

  // The SVG is built from untrusted Markdown, so only safe elements and URLs
  // are allowed through.
  var BLOCKED = {
    script: 1, foreignobject: 1, iframe: 1, object: 1, embed: 1,
    image: 1, feimage: 1, use: 1, audio: 1, video: 1, canvas: 1,
    animate: 1, set: 1
  };

  function safeUrl(value) {
    var url = String(value || '').trim().toLowerCase();
    return url.charAt(0) === '#' ||
      url.indexOf('http://') === 0 ||
      url.indexOf('https://') === 0 ||
      url.indexOf('./') === 0 ||
      url.indexOf('../') === 0 ||
      url.indexOf(':') === -1;
  }

  function cleanAttributes(element) {
    for (var i = element.attributes.length - 1; i >= 0; i--) {
      var attribute = element.attributes[i];
      var name = attribute.name.toLowerCase();
      if (name.indexOf('on') === 0 ||
          ((name === 'href' || name === 'xlink:href' || name === 'src') &&
            !safeUrl(attribute.value))) {
        element.removeAttributeNode(attribute);
      }
    }
  }

  function sanitizeSvg(markup) {
    var doc = new DOMParser().parseFromString(markup, 'image/svg+xml');
    if (doc.getElementsByTagName('parsererror').length) {
      throw new Error('The diagram renderer returned invalid SVG.');
    }
    var root = doc.documentElement;
    if (!root || root.localName.toLowerCase() !== 'svg') {
      throw new Error('The diagram renderer returned no SVG.');
    }
    var elements = root.querySelectorAll('*');
    for (var i = 0; i < elements.length; i++) {
      var element = elements[i];
      if (BLOCKED[element.localName.toLowerCase()]) {
        element.remove();
      } else {
        cleanAttributes(element);
      }
    }
    cleanAttributes(root);
    return root;
  }

  function fail(diagram, error) {
    var message = document.createElement('div');
    message.className = 'mermaid-error-message';
    message.textContent = error && error.message ? error.message : String(error);
    diagram.classList.add('mermaid-failed');
    diagram.insertBefore(message, diagram.firstChild);
  }

  var renderer;
  try {
    var data = document.getElementById('mhtml-mermaid-wasm');
    renderer = Ondine.create({ modules: { all: data.textContent.trim() } });
  } catch (error) {
    console.error('Failed to load the Mermaid renderer.', error);
    for (var i = 0; i < diagrams.length; i++) {
      fail(diagrams[i], new Error('Failed to load the Mermaid renderer.'));
    }
    return;
  }

  for (var i = 0; i < diagrams.length; i++) {
    var diagram = diagrams[i];
    try {
      var result = renderer.render(diagram.textContent);
      diagram.replaceChildren(document.importNode(sanitizeSvg(result.svg), true));
      diagram.classList.add('mermaid-rendered');
    } catch (error) {
      fail(diagram, error);
    }
  }
})();
