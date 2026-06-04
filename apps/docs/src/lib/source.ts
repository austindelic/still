import { docs } from "collections/server";
import { type InferPageType, loader } from "fumadocs-core/source";
import { lucideIconsPlugin } from "fumadocs-core/source/lucide-icons";

// See https://fumadocs.dev/docs/headless/source-api for more info
export const source = loader(docs.toFumadocsSource(), {
  baseUrl: "/docs",
  plugins: [lucideIconsPlugin()],
});

export function getPageData(page: InferPageType<typeof source>) {
  const data = docs.docs.find((doc) => doc.info.path === page.path);
  if (!data) {
    throw new Error(`Missing MDX data for page: ${page.path}`);
  }

  return data;
}

export function getPageImage(page: InferPageType<typeof source>) {
  const segments = [...page.slugs, "image.png"];

  return {
    segments,
    url: `/og/docs/${segments.join("/")}`,
  };
}

export async function getLLMText(page: InferPageType<typeof source>) {
  const data = getPageData(page);
  const processed = await data.getText("processed");

  return `# ${page.data.title}

${processed}`;
}
