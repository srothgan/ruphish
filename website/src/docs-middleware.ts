import { defineRouteMiddleware, type StarlightRouteData } from '@astrojs/starlight/route-data';
import { repository } from './site';

function capitalizeFolderLabels(entries: StarlightRouteData['sidebar']) {
  for (const entry of entries) {
    if (entry.type !== 'group') continue;
    if ('autogenerate' in entry) {
      entry.label = entry.label.charAt(0).toUpperCase() + entry.label.slice(1);
    }
    capitalizeFolderLabels(entry.entries);
  }
}

export const onRequest = defineRouteMiddleware((context) => {
  const route = context.locals.starlightRoute;
  capitalizeFolderLabels(route.sidebar);
  route.siteTitleHref = '/';
  const file = route.entry.filePath?.replaceAll('\\', '/').split('/content/docs/docs/')[1];
  if (file && route.entry.data.editUrl !== false) {
    route.editUrl = new URL(`${repository}/edit/main/docs/${file}`);
  }
});
