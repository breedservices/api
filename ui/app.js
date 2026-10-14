const endpoints = [
 {m:"GET",p:"/health",a:false,d:"Liveness probe.", params:[]},
 {m:"GET",p:"/spotify/token",a:false,d:"Fetch a Spotify access token.", params:[{n:"client_id",t:"string",r:false,desc:"Spotify client id (optional override)"},{n:"client_secret",t:"string",r:false,desc:"Spotify client secret (optional override)"}]},
 {m:"GET",p:"/religion/bible",a:false,d:"Bible verse lookup.", params:[{n:"book",t:"string",r:false,desc:"Book name"},{n:"chapter",t:"integer",r:false,desc:"Chapter number"},{n:"verse",t:"integer",r:false,desc:"Verse number"},{n:"translation",t:"string",r:false,desc:"Translation code"}]},
 {m:"GET",p:"/religion/bible/random",a:false,d:"Random Bible verse.", params:[]},
 {m:"GET",p:"/religion/quran",a:false,d:"Quran ayah lookup.", params:[{n:"surah",t:"integer",r:false,desc:"Surah number"},{n:"ayah",t:"integer",r:false,desc:"Ayah number"}]},
 {m:"GET",p:"/religion/quran/random",a:false,d:"Random ayah.", params:[]},
 {m:"GET",p:"/youtube/search",a:false,d:"Search YouTube videos.", params:[{n:"q",t:"string",r:true,desc:"Search query"},{n:"count",t:"integer",r:false,desc:"Max results (1-10)"},{n:"shorts",t:"boolean",r:false,desc:"Prefer shorts"}]},
 {m:"GET",p:"/youtube/video",a:false,d:"Fetch video metadata.", params:[{n:"url",t:"string",r:true,desc:"YouTube video URL"}]},
 {m:"GET",p:"/youtube/download",a:false,d:"Download media (binary).", params:[{n:"url",t:"string",r:true,desc:"Source URL"},{n:"audio",t:"boolean",r:false,desc:"Audio only (mp3)"}]},
 {m:"GET",p:"/soundcloud/search",a:false,d:"Search SoundCloud.", params:[{n:"q",t:"string",r:true,desc:"Search query"},{n:"count",t:"integer",r:false,desc:"Max results"}]},
 {m:"GET",p:"/soundcloud/track",a:false,d:"Track details.", params:[{n:"url",t:"string",r:true,desc:"Track URL"}]},
 {m:"GET",p:"/soundcloud/user",a:false,d:"User profile.", params:[{n:"url",t:"string",r:true,desc:"Profile URL"}]},
 {m:"GET",p:"/soundcloud/playlist",a:false,d:"Playlist details.", params:[{n:"url",t:"string",r:true,desc:"Playlist URL"}]},
 {m:"GET",p:"/soundcloud/usertracks",a:false,d:"User tracks.", params:[{n:"url",t:"string",r:true,desc:"Profile URL"},{n:"count",t:"integer",r:false,desc:"Max results"}]},
 {m:"GET",p:"/soundcloud/related",a:false,d:"Related tracks.", params:[{n:"url",t:"string",r:true,desc:"Track URL"}]},
 {m:"GET",p:"/soundcloud/download/audio",a:false,d:"Audio download (binary/file).", params:[{n:"url",t:"string",r:true,desc:"Track URL"},{n:"catbox",t:"boolean",r:false,desc:"Return JSON URL instead of binary"}]},
 {m:"GET",p:"/tiktok/latest",a:false,d:"Latest TikToks for a user.", params:[{n:"user_id",t:"string",r:true,desc:"Username (without @)"}]},
 {m:"GET",p:"/reddit/media",a:false,d:"Reddit media extraction.", params:[{n:"url",t:"string",r:true,desc:"Reddit post URL"}]},
 {m:"GET",p:"/reddit/download",a:false,d:"Reddit download (binary).", params:[{n:"url",t:"string",r:true,desc:"Post URL"},{n:"catbox",t:"boolean",r:false,desc:"Return JSON URL"}]},
 {m:"GET",p:"/utility/dominant-color",a:false,d:"Dominant color of an image.", params:[{n:"url",t:"string",r:true,desc:"Image URL"}]},
 {m:"GET",p:"/fun/quote/fonts",a:false,d:"Available quote fonts.", params:[]},
 {m:"POST",p:"/search/web",a:false,d:"Web search.", params:[{n:"q",t:"string",r:true,desc:"Search query"},{n:"count",t:"integer",r:false,desc:"Max results"}]},
 {m:"POST",p:"/search/images",a:false,d:"Image search.", params:[{n:"q",t:"string",r:true,desc:"Search query"},{n:"count",t:"integer",r:false,desc:"Max results"}]},
 {m:"POST",p:"/crypto/check",a:true,d:"Crypto address check (Bearer token required).", params:[{n:"address",t:"string",r:true,desc:"Address to analyze"},{n:"items",t:"array",r:false,desc:"Optional comparison items"}]},
 {m:"POST",p:"/utility/svg",a:false,d:"SVG render (image).", params:[{n:"text",t:"string",r:false,desc:"SVG markup"},{n:"url",t:"string",r:false,desc:"Source URL"}]},
 {m:"POST",p:"/utility/zip",a:false,d:"Zip utility (image).", params:[{n:"assets",t:"array",r:true,desc:"Assets to zip"},{n:"zip_filename",t:"string",r:false,desc:"Output filename"}]},
 {m:"POST",p:"/fun/caption",a:false,d:"Image captioning.", params:[{n:"url",t:"string",r:true,desc:"Image URL"},{n:"content",t:"string",r:false,desc:"Optional prompt"}]},
 {m:"POST",p:"/fun/img2gif",a:false,d:"Image to GIF.", params:[{n:"url",t:"string",r:true,desc:"Image URL"}]},
 {m:"POST",p:"/fun/vid2gif",a:false,d:"Video to GIF.", params:[{n:"url",t:"string",r:true,desc:"Video URL"}]},
 {m:"POST",p:"/fun/quote",a:false,d:"Quote image render.", params:[{n:"content",t:"string",r:true,desc:"Quote text"},{n:"username",t:"string",r:false,desc:"Author name"},{n:"avatar_url",t:"string",r:false,desc:"Avatar URL"},{n:"display_name",t:"string",r:false,desc:"Display name"},{n:"font",t:"string",r:false,desc:"Font filename"}]},
 {m:"POST",p:"/levelling/card",a:false,d:"Level card (image).", params:[{n:"username",t:"string",r:true,desc:"Username"},{n:"level",t:"integer",r:true,desc:"Level"},{n:"xp",t:"integer",r:false,desc:"XP"},{n:"progress_xp",t:"integer",r:false,desc:"Progress XP"},{n:"needed_xp",t:"integer",r:false,desc:"Needed XP"},{n:"avatar_url",t:"string",r:false,desc:"Avatar URL"}]},
 {m:"POST",p:"/levelling/levelup",a:false,d:"Level-up image.", params:[{n:"username",t:"string",r:true,desc:"Username"},{n:"old_level",t:"integer",r:true,desc:"Previous level"},{n:"new_level",t:"integer",r:true,desc:"New level"},{n:"avatar_url",t:"string",r:false,desc:"Avatar URL"}]},
 {m:"POST",p:"/levelling/leaderboard",a:false,d:"Leaderboard image.", params:[{n:"entries",t:"array",r:true,desc:"List of leaderboard entries"},{n:"title",t:"string",r:false,desc:"Title"},{n:"subtitle",t:"string",r:false,desc:"Subtitle"}]},
 {m:"POST",p:"/stats/dashboard",a:false,d:"Stats dashboard image.", params:[{n:"title",t:"string",r:false,desc:"Title"},{n:"subtitle",t:"string",r:false,desc:"Subtitle"},{n:"tables",t:"array",r:false,desc:"Tables data"},{n:"middle",t:"array",r:false,desc:"Middle row"},{n:"lookback_label",t:"string",r:false,desc:"Lookback label"},{n:"lookback_value",t:"string",r:false,desc:"Lookback value"}]},
];
const methods = [...new Set(endpoints.map(e=>e.m))];
const filters = document.getElementById('filters');
const allChip = document.createElement('button'); allChip.className='chip active'; allChip.textContent='All'; allChip.dataset.m='ALL'; filters.appendChild(allChip);
methods.forEach(m=>{const c=document.createElement('button');c.className='chip';c.textContent=m;c.dataset.m=m;filters.appendChild(c);});
let active='ALL';
filters.addEventListener('click',e=>{
  const t=e.target.closest('.chip'); if(!t) return;
  active=t.dataset.m;
  document.querySelectorAll('.chip').forEach(c=>c.classList.toggle('active',c===t));
  render();
});
const search=document.getElementById('search');
const grid=document.getElementById('grid');
function render(){
  const q=search.value.trim().toLowerCase();
  grid.innerHTML='';
  endpoints.filter(e=>{
    if(active!=='ALL' && e.m!==active) return false;
    if(!q) return true;
    return (e.m+e.p+e.d+e.params.map(x=>x.n+x.desc).join(' ')).toLowerCase().includes(q);
  }).forEach(e=>{
    const card=document.createElement('div'); card.className='card';
    const head=document.createElement('div'); head.className='card-head';
    head.innerHTML=`<span class="method ${e.m}">${e.m}</span><span class="path">${e.p}</span><span class="${e.a?'auth':'open'}">${e.a?'auth':'open'}</span><span class="toggle">▾</span>`;
    const desc=document.createElement('div'); desc.className='desc'; desc.textContent=e.d;
    const details=document.createElement('div'); details.className='details';
    if(e.params.length){
      details.innerHTML = e.params.map(pr=>{
        return `<div class="param"><div><span class="pname">${pr.n}</span><span class="ptype">${pr.t}${pr.r?' · required':''}</span></div><div class="pdesc">${pr.desc}</div></div>`;
      }).join('');
    } else {
      details.innerHTML = '<div class="param"><div class="pdesc">No documented parameters.</div></div>';
    }
    head.addEventListener('click',()=>details.classList.toggle('open'));
    card.appendChild(head); card.appendChild(desc); card.appendChild(details);
    grid.appendChild(card);
  });
}
search.addEventListener('input',render);
render();
EOF
echo done