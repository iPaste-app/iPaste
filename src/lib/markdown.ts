import MarkdownIt from "markdown-it";

// Release notes are remote content: keep embedded HTML escaped.
const markdown = new MarkdownIt({ html: false, linkify: true });

export function renderMarkdown(source: string) {
  return markdown.render(source);
}
