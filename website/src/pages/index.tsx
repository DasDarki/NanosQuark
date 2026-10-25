import type {ReactNode} from 'react';
import Link from '@docusaurus/Link';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import CodeBlock from '@theme/CodeBlock';
import Layout from '@theme/Layout';
import Atom from '@site/src/components/Atom';
import styles from './index.module.css';

type Category = {
  title: string;
  description: string;
  modules: string[];
  to: string;
  accent: 'cyan' | 'violet' | 'magenta';
};

const CATEGORIES: Category[] = [
  {
    title: 'Crypto and security',
    description: 'Hashing, Argon2 passwords, AES and ChaCha encryption, signatures, JWT and one time passwords.',
    modules: ['Crypto'],
    to: '/docs/crypto',
    accent: 'magenta',
  },
  {
    title: 'Networking',
    description: 'HTTP servers with routing, WebSockets, TCP and UDP sockets, Redis, MQTT and NATS.',
    modules: ['HTTP', 'WebSocket', 'TCP', 'UDP', 'Redis', 'MQTT', 'NATS'],
    to: '/docs/net',
    accent: 'cyan',
  },
  {
    title: 'Data and formats',
    description: 'JSON, TOML, YAML, MessagePack, CBOR, XML, CSV, compression and archives in memory.',
    modules: ['JSON', 'YAML', 'CSV', 'Compression', 'Zip', 'Buffer'],
    to: '/docs/formats',
    accent: 'violet',
  },
  {
    title: 'Native computation',
    description: 'Pathfinding, spatial queries, procedural generation, images and real parallel Lua in workers.',
    modules: ['Pathfinding', 'Spatial', 'ProcGen', 'Image', 'Worker'],
    to: '/docs/spatial',
    accent: 'magenta',
  },
  {
    title: 'Math and randomness',
    description: 'Quaternions, curves, statistics, safe formulas, geometry, big integers, decimals and seeded noise.',
    modules: ['Math', 'Geometry', 'BigInt', 'Decimal', 'Random'],
    to: '/docs/math',
    accent: 'cyan',
  },
  {
    title: 'Text, time and tooling',
    description: 'Regex, templates, word filters, time zones and cron, translations, caches, metrics and tests.',
    modules: ['Regex', 'Template', 'Time', 'Locale', 'Cache', 'Testing'],
    to: '/docs/text',
    accent: 'violet',
  },
];

const PRINCIPLES = [
  {
    title: 'Off the game thread',
    text: 'Heavy work runs on native worker threads. Results come back as callbacks, promises or awaits, without stalling the tick.',
  },
  {
    title: 'Safe by default',
    text: 'Linear time regex for player input, sandboxed workers, network allow lists and errors with the script location.',
  },
  {
    title: 'Free when unused',
    text: 'Modules are built on first access. Leave entire modules out at compile time with Cargo features.',
  },
];

const ASYNC_EXAMPLE = `local Crypto = Quark.Import("Crypto")

Player.Subscribe("Spawn", function(player)
    Quark.Async(function()
        local hash = Quark.Await(Crypto.Argon2Hash, "secret")
        Console.Log("stored " .. hash)
    end)
end)`;

const SERVER_EXAMPLE = `local server = Quark.HTTP.Listen(8080)

server:Get("/players", function(request, response)
    local names = {}
    for _, player in ipairs(Player.GetAll()) do
        table.insert(names, player:GetName())
    end
    response:Json(names)
end)`;

function Hero(): ReactNode {
  const {siteConfig} = useDocusaurusContext();
  return (
    <header className={styles.hero}>
      <div className={styles.orbitLines} aria-hidden="true" />
      <div className={`container ${styles.heroInner}`}>
        <div className={styles.heroText}>
          <p className={styles.eyebrow}>Rust powered c-module for nanos world</p>
          <h1 className={styles.title}>
            <span className={styles.titleNanos}>Nanos</span>
            <span className={styles.titleQuark}>Quark</span>
          </h1>
          <p className={styles.tagline}>{siteConfig.tagline}</p>
          <p className={styles.lead}>
            Cryptography, networking, data formats, compression and fast native computation for your server scripts,
            all from one global <code>Quark</code>.
          </p>
          <div className={styles.actions}>
            <Link className={styles.primaryButton} to="/docs/introduction">
              Get started
            </Link>
            <Link className={styles.secondaryButton} to="/docs/introduction#modules">
              Browse modules
            </Link>
          </div>
        </div>
        <div className={styles.heroArt}>
          <Atom />
        </div>
      </div>
    </header>
  );
}

function Categories(): ReactNode {
  return (
    <section className={styles.section}>
      <div className="container">
        <h2 className={styles.sectionTitle}>Everything the scripting API leaves out</h2>
        <p className={styles.sectionLead}>Over forty modules, documented with examples and typed for the Lua Language Server and Nebra.</p>
        <div className={styles.grid}>
          {CATEGORIES.map((category) => (
            <Link key={category.title} to={category.to} className={`${styles.card} ${styles[category.accent]}`}>
              <span className={styles.cardDot} aria-hidden="true" />
              <h3>{category.title}</h3>
              <p>{category.description}</p>
              <ul className={styles.chips}>
                {category.modules.map((module) => (
                  <li key={module}>{module}</li>
                ))}
              </ul>
            </Link>
          ))}
        </div>
      </div>
    </section>
  );
}

function Showcase(): ReactNode {
  return (
    <section className={styles.section}>
      <div className="container">
        <div className={styles.principles}>
          {PRINCIPLES.map((principle) => (
            <div key={principle.title} className={styles.principle}>
              <h3>{principle.title}</h3>
              <p>{principle.text}</p>
            </div>
          ))}
        </div>
        <div className={styles.examples}>
          <div>
            <h3 className={styles.exampleTitle}>Await native work</h3>
            <CodeBlock language="lua">{ASYNC_EXAMPLE}</CodeBlock>
          </div>
          <div>
            <h3 className={styles.exampleTitle}>Serve HTTP from the game server</h3>
            <CodeBlock language="lua">{SERVER_EXAMPLE}</CodeBlock>
          </div>
        </div>
      </div>
    </section>
  );
}

function CallToAction(): ReactNode {
  return (
    <section className={styles.cta}>
      <div className="container">
        <h2>Add Quark to your server</h2>
        <p>Copy the package, require it and start the server with unsafe libraries enabled.</p>
        <Link className={styles.primaryButton} to="/docs/installation">
          Installation guide
        </Link>
      </div>
    </section>
  );
}

export default function Home(): ReactNode {
  const {siteConfig} = useDocusaurusContext();
  return (
    <Layout title={siteConfig.tagline} description="NanosQuark adds cryptography, networking, data formats and native computation to the nanos world server.">
      <Hero />
      <main>
        <Categories />
        <Showcase />
        <CallToAction />
      </main>
    </Layout>
  );
}
