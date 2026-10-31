import { getCollection } from 'astro:content';

// Drafts render in development and stay out of production builds and the sitemap.
export async function publishedPosts() {
  const posts = await getCollection('blog', ({ data }) => import.meta.env.DEV || !data.draft);
  return posts.sort((a, b) => b.data.date.valueOf() - a.data.date.valueOf());
}

// Tags cycle through the badge styles in alphabetical order, so a tag keeps its style on every page.
// The negative style is excluded because red signals errors, not a topic.
const tagStyles = ['neutral', 'positive', 'warning', 'active'] as const;
export async function tagStyle() {
  const tags = [...new Set((await publishedPosts()).flatMap((post) => post.data.tags))].sort();
  return (tag: string) => tagStyles[tags.indexOf(tag) % tagStyles.length];
}

const dateFormat = new Intl.DateTimeFormat('en', { dateStyle: 'long', timeZone: 'UTC' });
export const formatDate = (date: Date) => dateFormat.format(date);
