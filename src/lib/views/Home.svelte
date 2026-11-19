<script lang="ts">
  import { Plus, Clock, Sparkles, ArrowRight, FolderInput } from '@lucide/svelte';
  import InstanceArt from '$lib/components/InstanceArt.svelte';
  import InstanceCard from '$lib/components/InstanceCard.svelte';
  import PlayButton from '$lib/components/PlayButton.svelte';
  import Progress from '$lib/components/Progress.svelte';
  import { app, go, activeAccount, playtime, ago, isBusy, openInstanceMenu } from '$lib/state.svelte';
  import { loaderLabel } from '$lib/types';

  const hero = $derived(app.instances[0]);
  // Brand artwork (static/hero.jpg); falls back to generated art if missing.
  let heroImg = $state(true);
  const rest = $derived(app.instances.slice(1, 7));
  const account = $derived(activeAccount());

  const greeting = (() => {
    const h = new Date().getHours();
    return h < 5 ? 'Late night session' : h < 12 ? 'Good morning' : h < 18 ? 'Good afternoon' : 'Good evening';
  })();
</script>

<div class="home">
  <div class="greet rise">
    <h1>{greeting}{account ? `, ${account.username}` : ''}</h1>
    <p>{hero ? 'Pick up right where you left off.' : "Let's get your first world set up."}</p>
  </div>

  {#if hero}
    <section class="hero glass rise" style:animation-delay="60ms" oncontextmenu={(e) => openInstanceMenu(e, hero)} aria-label={hero.name}>
      <div class="hero-art">
        {#if heroImg}
          <img src="/hero.jpg" alt="" onerror={() => (heroImg = false)} />
        {:else}
          <InstanceArt loader={hero.loader} image={hero.pack?.bannerUrl} icon={hero.pack?.iconUrl} />
        {/if}
      </div>
      <div class="hero-shade"></div>
      <div class="hero-content">
        <span class="label eyebrow">{hero.lastPlayed ? 'Jump back in' : 'Ready when you are'}</span>
        <h2>{hero.name}</h2>
        <div class="hero-meta">
          <span class="chip">Minecraft {hero.gameVersion}</span>
          <span class="chip">{loaderLabel(hero.loader)}{hero.loaderVersion ? ` ${hero.loaderVersion}` : ''}</span>
          <span class="stat"><Clock size={13} /> {playtime(hero.playtimeSecs)}</span>
          <span class="stat">Last played {ago(hero.lastPlayed).toLowerCase()}</span>
        </div>
        <div class="hero-actions">
          <PlayButton inst={hero} />
          <button class="btn lg" onclick={() => go('instance', hero.id)}>Details <ArrowRight size={16} /></button>
        </div>
        {#if isBusy(hero.id) && app.progress[hero.id]}
          <div class="hero-progress"><Progress p={app.progress[hero.id]} /></div>
        {/if}
      </div>
    </section>
  {:else}
    <section class="welcome glass rise" class:with-art={heroImg}>
      {#if heroImg}<img class="welcome-bg" src="/hero.jpg" alt="" onerror={() => (heroImg = false)} />{/if}
      <div class="welcome-icon"><Sparkles size={26} /></div>
      <h2>Welcome to <span class="gradient-text">Peak</span></h2>
      <p>Create an instance and we'll download the game, its assets and the right Java version for you. No setup needed.</p>
      <div class="welcome-actions">
        <button class="btn primary lg" onclick={() => (app.creating = true)}><Plus size={18} /> Create your first instance</button>
        <button class="btn lg" onclick={() => (app.importing = true)}><FolderInput size={18} /> Import from another launcher</button>
      </div>
    </section>
  {/if}

  {#if hero}
    <section class="recent">
      <div class="section-head">
        <h3>Your instances</h3>
        <button class="btn ghost" onclick={() => go('library')}>View all <ArrowRight size={15} /></button>
      </div>
      <div class="grid">
        {#each rest as inst, i (inst.id)}
          <InstanceCard {inst} index={i} />
        {/each}
        <button class="new glass rise" style:animation-delay="{rest.length * 40}ms" onclick={() => (app.creating = true)}>
          <span class="plus"><Plus size={22} /></span>
          <span>New instance</span>
        </button>
      </div>
    </section>
  {/if}
</div>

<style>
  .home {
    display: flex;
    flex-direction: column;
    gap: 26px;
  }
  .greet h1 {
    font-size: 28px;
    font-weight: 720;
  }
  .greet p {
    margin: 6px 0 0;
    color: var(--muted);
    font-size: 14px;
  }
  .hero {
    position: relative;
    min-height: 300px;
    overflow: hidden;
    padding: 0;
    display: flex;
    align-items: flex-end;
  }
  .hero-art img,
  .welcome-bg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: 60% 40%;
  }
  .welcome {
    position: relative;
    overflow: hidden;
  }
  .welcome > :not(.welcome-bg) {
    position: relative;
  }
  .welcome.with-art::before {
    content: '';
    position: absolute;
    inset: 0;
    z-index: 1;
    background: radial-gradient(ellipse at center, rgba(7, 8, 13, 0.55), rgba(7, 8, 13, 0.85));
  }
  .welcome.with-art > :not(.welcome-bg) {
    z-index: 2;
  }
  .hero-art {
    position: absolute;
    inset: 0;
  }
  .hero-shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, rgba(7, 8, 13, 0.92) 0%, rgba(7, 8, 13, 0.55) 45%, rgba(7, 8, 13, 0.05) 100%),
      linear-gradient(0deg, rgba(7, 8, 13, 0.6), transparent 60%);
  }
  .hero-content {
    position: relative;
    padding: 34px;
    max-width: 620px;
  }
  .eyebrow {
    color: var(--accent-2);
  }
  .hero h2 {
    font-size: 38px;
    font-weight: 760;
    margin: 8px 0 14px;
    line-height: 1.05;
  }
  .hero-meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 14px;
    margin-bottom: 24px;
  }
  .hero-meta .chip {
    background: rgba(255, 255, 255, 0.1);
    color: var(--text);
  }
  .stat {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .hero-actions {
    display: flex;
    gap: 10px;
  }
  .hero-progress {
    margin-top: 18px;
    max-width: 380px;
  }
  .welcome {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    padding: 56px 32px;
    gap: 14px;
  }
  .welcome-icon {
    width: 58px;
    height: 58px;
    border-radius: 18px;
    display: grid;
    place-items: center;
    color: var(--on-accent);
    background: var(--gradient);
  }
  .welcome h2 {
    font-size: 28px;
    font-weight: 740;
  }
  .welcome p {
    margin: 0 0 8px;
    max-width: 440px;
    color: var(--muted);
    line-height: 1.55;
    font-size: 14px;
  }
  .welcome-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 10px;
  }
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 14px;
  }
  .section-head h3 {
    font-size: 17px;
    font-weight: 650;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
    gap: 16px;
  }
  .new {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    min-height: 200px;
    color: var(--muted);
    font-weight: 560;
    font-size: 13.5px;
    border-style: dashed;
    background: rgba(255, 255, 255, 0.02);
    transition: background 0.25s, color 0.25s, border-color 0.25s;
  }
  .new:hover {
    color: var(--text);
    background: rgba(255, 255, 255, 0.05);
    border-color: color-mix(in srgb, var(--accent) 50%, transparent);
  }
  .plus {
    width: 46px;
    height: 46px;
    border-radius: 14px;
    display: grid;
    place-items: center;
    background: rgba(255, 255, 255, 0.06);
  }
</style>
