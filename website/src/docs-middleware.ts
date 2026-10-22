import { defineRouteMiddleware } from '@astrojs/starlight/route-data';
import { repository } from './site';

export const onRequest = defineRouteMiddleware((context) => {
  const route = context.locals.starlightRoute;
  route.siteTitleHref = '/';
  const file = route.entry.filePath?.replaceAll('\\', '/').split('/content/docs/docs/')[1];
  if (file && route.entry.data.editUrl !== false) {
    route.editUrl = new URL(`${repository}/edit/main/docs/${file}`);
  }
});
