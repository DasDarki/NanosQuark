import type * as Preset from '@docusaurus/preset-classic';
import type {Config} from '@docusaurus/types';
import {themes as prismThemes} from 'prism-react-renderer';

const GITHUB_OWNER = 'DasDarki';
const GITHUB_REPO = 'NanosQuark';
const REPOSITORY = `https://github.com/${GITHUB_OWNER}/${GITHUB_REPO}`;

const config: Config = {
  title: 'NanosQuark',
  tagline: 'Native utilities for nanos world',
  favicon: 'img/favicon.svg',

  url: `https://${GITHUB_OWNER.toLowerCase()}.github.io`,
  baseUrl: `/${GITHUB_REPO}/`,

  organizationName: GITHUB_OWNER,
  projectName: GITHUB_REPO,
  deploymentBranch: 'gh-pages',
  trailingSlash: false,

  onBrokenLinks: 'throw',
  onBrokenAnchors: 'warn',

  markdown: {
    hooks: {
      onBrokenMarkdownLinks: 'throw',
    },
  },

  future: {
    v4: true,
    faster: true,
  },

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  stylesheets: [
    {
      href: 'https://fonts.googleapis.com/css2?family=Montserrat:wght@500;600;700;800&family=Inter:wght@400;500;600&family=JetBrains+Mono:wght@400;500&display=swap',
      type: 'text/css',
    },
  ],

  presets: [
    [
      'classic',
      {
        docs: {
          path: '../docs',
          sidebarPath: './sidebars.ts',
          routeBasePath: 'docs',
          editUrl: ({docPath}) => `${REPOSITORY}/edit/main/docs/${docPath}`,
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
        sitemap: {
          changefreq: 'weekly',
          priority: 0.5,
        },
      } satisfies Preset.Options,
    ],
  ],

  plugins: [
    [
      require.resolve('@easyops-cn/docusaurus-search-local'),
      {
        hashed: true,
        indexBlog: false,
        docsDir: '../docs',
        docsRouteBasePath: '/docs',
        highlightSearchTermsOnTargetPage: true,
        explicitSearchResultPath: true,
        searchBarShortcut: true,
        searchBarShortcutHint: true,
        searchResultLimits: 12,
        searchResultContextMaxLength: 60,
      },
    ],
  ],

  themeConfig: {
    image: 'img/social-card.jpg',
    metadata: [
      {
        name: 'keywords',
        content: 'nanos world, lua, rust, c-module, server, utilities, crypto, http, redis, pathfinding',
      },
    ],

    colorMode: {
      defaultMode: 'dark',
      disableSwitch: false,
      respectPrefersColorScheme: false,
    },

    navbar: {
      title: 'NanosQuark',
      logo: {
        alt: 'NanosQuark logo',
        src: 'img/logo.svg',
      },
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'docsSidebar',
          position: 'left',
          label: 'Docs',
        },
        {
          to: '/docs/installation',
          position: 'left',
          label: 'Install',
        },
        {
          to: '/docs/introduction#modules',
          position: 'left',
          label: 'Modules',
        },
        {
          href: REPOSITORY,
          position: 'right',
          className: 'navbar-github-link',
          'aria-label': 'GitHub repository',
        },
      ],
    },

    footer: {
      style: 'dark',
      links: [
        {
          title: 'Learn',
          items: [
            {label: 'Introduction', to: '/docs/introduction'},
            {label: 'Installation', to: '/docs/installation'},
            {label: 'Basics', to: '/docs/basics'},
            {label: 'Configuration', to: '/docs/configuration'},
          ],
        },
        {
          title: 'Modules',
          items: [
            {label: 'Crypto', to: '/docs/crypto'},
            {label: 'Networking', to: '/docs/net'},
            {label: 'Math', to: '/docs/math'},
            {label: 'Worker', to: '/docs/worker'},
          ],
        },
        {
          title: 'Project',
          items: [
            {label: 'GitHub', href: REPOSITORY},
            {label: 'Issues', href: `${REPOSITORY}/issues`},
            {label: 'nanos world', href: 'https://nanos.world'},
            {label: 'Nebra', href: 'https://nebra-lang.github.io'},
          ],
        },
      ],
      copyright: `Copyright &copy; ${new Date().getFullYear()} DasDarki. Not affiliated with nanos world.`,
    },

    docs: {
      sidebar: {
        hideable: true,
        autoCollapseCategories: false,
      },
    },

    tableOfContents: {
      minHeadingLevel: 2,
      maxHeadingLevel: 3,
    },

    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      defaultLanguage: 'lua',
      additionalLanguages: ['lua', 'toml', 'bash', 'json', 'ini', 'rust'],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
