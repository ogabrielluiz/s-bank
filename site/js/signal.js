/* ============================================================
   SAM-e / S-  —  signal engine
   ============================================================ */
(function(){
  'use strict';

  /* ---------- scroll reveal ---------- */
  const io = new IntersectionObserver((es)=>{
    es.forEach(e=>{ if(e.isIntersecting){ e.target.classList.add('in'); io.unobserve(e.target);} });
  },{threshold:.16});
  function observeRises(){ document.querySelectorAll('.rise:not(.in)').forEach(el=>io.observe(el)); }

  /* ---------- nav flips to dark over dark sections ---------- */
  const nav = document.querySelector('.nav');
  const darkSecs = [...document.querySelectorAll('section.dark')];
  function navState(){
    if(!nav) return;
    const y = window.innerHeight*0.10;
    let over=false;
    darkSecs.forEach(s=>{ const r=s.getBoundingClientRect(); if(r.top<=y && r.bottom>=y) over=true; });
    nav.classList.toggle('on-dark', over);
  }

  /* ---------- hyphen modes (quiet texture) ---------- */
  const MODES = ['-','~',':','>','•','|','→'];
  document.querySelectorAll('[data-cycle]').forEach(el=>{
    let i=0;
    const base = el.dataset.cycle || '';
    function set(g){ el.innerHTML = base + '<span class="glyph">'+g+'</span>'; }
    set(MODES[0]);
    el.addEventListener('mouseenter',()=>{ i=(i+1)%MODES.length; set(MODES[i]); });
  });
  // ambient slow drift on any [data-drift] hyphen
  document.querySelectorAll('[data-drift]').forEach((el,k)=>{
    let i=0; const base=el.dataset.drift||'';
    el.innerHTML = base+'<span class="glyph">-</span>';
    const g=()=>el.querySelector('.glyph');
    setInterval(()=>{ i=(i+1)%MODES.length; if(g()) g().textContent=MODES[i]; }, 2600 + k*430);
  });

  /* ---------- oscilloscope / signal canvases ---------- */
  function scope(canvas){
    const ctx = canvas.getContext('2d');
    const dpr = Math.min(window.devicePixelRatio||1, 2);
    let w,h;
    function size(){ const r=canvas.getBoundingClientRect(); w=r.width; h=r.height; canvas.width=w*dpr; canvas.height=h*dpr; ctx.setTransform(dpr,0,0,dpr,0,0); }
    size(); new ResizeObserver(size).observe(canvas);
    const color = canvas.dataset.color || '#FF5A00';
    const mode = canvas.dataset.mode || 'wave';
    let t=0, energy=0;
    canvas.addEventListener('pointermove',()=>{ energy=Math.min(1,energy+.12); });
    function frame(){
      t+=0.018; energy*=0.96;
      ctx.clearRect(0,0,w,h);
      const mid=h/2;
      ctx.lineWidth=1.6; ctx.strokeStyle=color; ctx.globalAlpha=1;
      ctx.beginPath();
      for(let x=0;x<=w;x+=2){
        const p=x/w;
        let y;
        const amp = h*0.30*(0.5+energy*0.7);
        if(mode==='wave'){
          y = mid + Math.sin(p*22 + t*3)*amp*Math.sin(p*Math.PI) ;
        }else if(mode==='clock'){
          y = mid + (Math.sign(Math.sin(p*40 + t*4))||0)*amp*0.7*Math.sin(p*Math.PI);
        }else if(mode==='data'){
          y = mid + (Math.round(Math.sin(p*30+t*2)*3)/3)*amp*Math.sin(p*Math.PI);
        }else{ y = mid + Math.sin(p*16+t*2)*amp; }
        x===0?ctx.moveTo(x,y):ctx.lineTo(x,y);
      }
      ctx.stroke();
      // baseline ticks
      ctx.globalAlpha=.25; ctx.strokeStyle=color; ctx.lineWidth=1;
      for(let x=0;x<w;x+=Math.max(28,w/18)){ ctx.beginPath(); ctx.moveTo(x,mid-3); ctx.lineTo(x,mid+3); ctx.stroke(); }
      ctx.globalAlpha=1;
      requestAnimationFrame(frame);
    }
    frame();
  }
  document.querySelectorAll('canvas[data-scope]').forEach(scope);

  /* ---------- signal-acquired intro ---------- */
  function bootIntro(){
    const root = document.getElementById('intro');
    if(!root) return;
    const log = root.querySelector('#bootlog');
    const lines = [
      ['> init system', 80],
      ['  source ... SAM-e', 60],
      ['  mode ..... LIVE', 60],
      ['  seq ...... 001', 60],
      ['  tempo .... 128.0', 60],
      ['> acquiring signal', 120],
      ['  ........ <span class="ok">LOCKED</span>', 220],
      ['> S- is the constant. you are the variable.', 300]
    ];
    let i=0;
    function next(){
      if(i>=lines.length){ root.classList.add('locked'); observeRises(); return; }
      const [txt,delay]=lines[i++];
      const d=document.createElement('div'); d.innerHTML=txt; log.appendChild(d);
      setTimeout(next, delay);
    }
    setTimeout(next, 500);
  }

  /* ---------- color = state interaction ---------- */
  window.setSignalState = function(state, el){
    const stage = document.getElementById('colorstage');
    if(!stage) return;
    const map = {
      orange:['#FF5A00','SIGNAL','active / alive / transmitting','S- · status · important marks'],
      yellow:['#FFC400','ENERGY','peak / intensity / discovery','live visuals · impact · highlights'],
      cyan:['#19D2E5','INFORMATION','data / comms / clarity / flow','UI · sequences · diagrams · motion'],
      eblue:['#1C46FF','DEPTH','electricity / unknown / night','artwork · atmosphere · immersive']
    };
    const [c,name,mean,use]=map[state];
    stage.style.setProperty('--sig',c);
    stage.querySelector('.sig-name').textContent=name;
    stage.querySelector('.sig-mean').textContent=mean;
    stage.querySelector('.sig-use').textContent=use;
    stage.querySelectorAll('.sig-canvas').forEach(cv=>cv.dataset.color=c);
    document.querySelectorAll('[data-state]').forEach(b=>b.classList.toggle('on', b===el));
  };

  /* ---------- run ---------- */
  window.addEventListener('scroll', ()=>{ navState(); }, {passive:true});
  observeRises(); navState(); bootIntro();
})();
