import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docsSidebar: [
    {
      type: 'category',
      label: 'Getting started',
      collapsible: false,
      items: ['introduction', 'installation', 'basics', 'configuration', 'editor-support'],
    },
    {
      type: 'category',
      label: 'Core',
      items: ['buffer', 'promise'],
    },
    {
      type: 'category',
      label: 'Data',
      items: ['formats', 'compression', 'crypto'],
    },
    {
      type: 'category',
      label: 'Networking',
      items: ['net', 'messaging', 'redis'],
    },
    {
      type: 'category',
      label: 'Computation',
      items: ['math', 'spatial', 'procgen', 'random', 'image', 'worker'],
    },
    {
      type: 'category',
      label: 'Text and time',
      items: ['text', 'time', 'locale'],
    },
    {
      type: 'category',
      label: 'Utilities',
      items: ['utilities', 'testing'],
    },
  ],
};

export default sidebars;
