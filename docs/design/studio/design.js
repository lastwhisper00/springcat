(() => {
  'use strict';
  const data=window.SPRINGCAT_DESIGN;
  const $=s=>document.querySelector(s);
  let page='placement',theme='graphite',selected=null;
  const names={graphite:'深墨',porcelain:'纸白',companion:'深海灰'};
  const notes={placement:'Concept 09：自托管 Noto Sans SC，任务标题15、辅助与状态13。悬停只显示整行半透明背景，文字与图标颜色保持不变；来源球与同级列表保留。',states:'待确认仅是一种状态，与其他任务同级。列表按最近更新排列，阅读期间不重排；紧凑入口通知规则单独定义。',workspace:'现有任务监听保留；详情、历史筛选和跳转失败反馈属于设计补齐。未来动作入口独立标注。',settings:'保留现有设置和 8 个来源接入。主题、减少动态效果与保存失败反馈属于新增设计。',onboarding:'首次使用向导是新增设计。允许跳过，并在设置中随时连接工具或调整位置。',usage:'图中所有用量都是示例。API 等价估算不代表订阅账单；各来源的数据覆盖不同。',style:'深墨、纸白、深海灰共享 Noto Sans SC 字体、尺寸与交互。悬停仅改变整行背景，文字、图标与状态颜色保持稳定。',flows:'这些流程定义产品应如何响应用户。已存在的能力与新的交互策略分别标记。'};
  const escape=s=>String(s).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const tagClass=a=>a.availability.includes('未来')?'future':a.availability.includes('提案')||a.availability.includes('补齐')?'proposed':'';
  function meta(a){return `<span class="badge ${tagClass(a)}">${escape(a.availability)}</span>`;}
  function annotations(a){return `<div class="notes"><div class="selected-id">${escape(a.id)}</div><h2>${escape(a.title)}</h2>${meta(a)}<dl><dt>触发 / 入口</dt><dd>${escape(a.entry)}</dd><dt>主要交互</dt><dd>${escape(a.action)}</dd><dt>收起 / 返回</dt><dd>${escape(a.exit)}</dd><dt>产品约束</dt><dd>${escape(a.note)}</dd></dl></div>`;}
  function downloadLink(a){return `artboards/${a.id}-${theme}.svg`;}
  function select(id,open=false){
    selected=data.boards.find(a=>a.id===id);if(!selected)return;
    document.querySelectorAll('.frame-button').forEach(el=>el.classList.toggle('selected',el.dataset.id===id));
    $('#inspector').innerHTML=annotations(selected)+`<div class="size"><span>W ${selected.width}</span><span>H ${selected.height}</span></div><button class="inspect-open">放大查看 ↗</button><a class="inspect-download" href="${downloadLink(selected)}" download>下载 ${names[theme]} SVG ↓</a><hr><div class="note-small">画板尺寸是设计展示尺寸。组件逻辑尺寸见画板内标注与视觉规范。<br>所有动作与数据为演示，未修改真实应用设置。</div>`;
    $('.inspect-open').addEventListener('click',openDetail);
    if(open)openDetail();
  }
  function openDetail(){if(!selected)return;$('#detail-id').textContent=selected.id+' · '+names[theme];$('#detail-title').textContent=selected.title;$('#stage').innerHTML=selected.svgs[theme];$('#detail-notes').innerHTML=annotations(selected);$('#download-svg').href=downloadLink(selected);$('#download-svg').download=`${selected.id}-${theme}.svg`;if(!$('#detail').open)$('#detail').showModal();requestAnimationFrame(scaleDetail);}
  function scaleDetail(){const svg=$('#stage svg');if(!svg||!selected)return;let scale=$('#detail-scale').value;if(scale==='fit')scale=Math.min(1,($('#stage').clientWidth-48)/selected.width,matchMedia('(min-width:701px)').matches?($('#stage').clientHeight-48)/selected.height:1);else scale=Number(scale);scale=Math.max(.2,Number(scale));svg.style.width=`${selected.width*scale}px`;svg.style.height=`${selected.height*scale}px`;}
  function render(){
    const p=data.pages.find(p=>p.id===page),q=$('#search').value.trim().toLowerCase();
    $('#page-title').textContent=p.name;$('#page-subtitle').textContent=p.subtitle;$('#page-number').textContent=`PAGE ${String(data.pages.indexOf(p)+1).padStart(2,'0')} / SPRINGCAT`;$('#page-note').textContent=notes[page];$('#mobile-page').value=page;
    $('#download-sheet').href=`sheets/${String(data.pages.indexOf(p)+1).padStart(2,'0')}-${page}-${theme}.svg`;
    document.querySelectorAll('.page-button').forEach(b=>{b.classList.toggle('active',b.dataset.page===page);b.setAttribute('aria-current',b.dataset.page===page?'page':'false')});
    const all=data.boards.filter(a=>a.page===page),boards=all.filter(a=>(a.title+' '+a.id+' '+a.availability).toLowerCase().includes(q));
    $('#count').textContent=boards.length+' / '+all.length+' 块画板';$('#empty').hidden=boards.length>0;
    $('#boards').innerHTML=boards.map(a=>`<article class="artboard"><div class="artboard-title"><span>${escape(a.title)}</span><span class="id">${escape(a.id)}</span></div><button class="frame-button" data-id="${a.id}" aria-label="查看 ${escape(a.title)}">${a.svgs[theme]}</button><div class="artboard-meta">${meta(a)}<span>${a.width} × ${a.height}</span></div></article>`).join('');
    document.querySelectorAll('.frame-button').forEach(b=>{b.addEventListener('click',()=>select(b.dataset.id,matchMedia('(max-width:1150px)').matches));b.addEventListener('dblclick',()=>select(b.dataset.id,true));b.addEventListener('keydown',e=>{if(e.key==='Enter'){e.preventDefault();select(b.dataset.id,true)}})});
    if(boards.length)select(boards.some(a=>a.id===selected?.id)?selected.id:boards[0].id);
    $('#totals').textContent=`${data.boards.length} 块画板 · ${data.pages.length} 组页面 · 3 套主题 · ${data.boards.length*3} 个 SVG`;
  }
  function changePage(id){page=id;$('#search').value='';render();window.scrollTo({top:0,behavior:'instant'});history.replaceState(null,'','#'+page);}
  $('#pages').innerHTML=data.pages.map((p,i)=>`<button class="page-button" data-page="${p.id}"><span class="number">${String(i+1).padStart(2,'0')}</span>${escape(p.name)}<span class="pc">${data.boards.filter(a=>a.page===p.id).length}</span></button>`).join('');
  $('#mobile-page').innerHTML=data.pages.map(p=>`<option value="${p.id}">${escape(p.name)}</option>`).join('');
  document.querySelectorAll('.page-button').forEach(b=>b.addEventListener('click',()=>changePage(b.dataset.page)));
  $('#mobile-page').addEventListener('change',e=>changePage(e.target.value));$('#theme').addEventListener('change',e=>{theme=e.target.value;render();if($('#detail').open)openDetail()});$('#density').addEventListener('change',e=>$('#boards').dataset.density=e.target.value);$('#search').addEventListener('input',render);$('#clear-search').addEventListener('click',()=>{$('#search').value='';render()});$('#close-detail').addEventListener('click',()=>$('#detail').close());$('#detail-scale').addEventListener('change',scaleDetail);window.addEventListener('resize',scaleDetail);$('#detail').addEventListener('click',e=>{if(e.target===$('#detail'))$('#detail').close()});
  const hash=location.hash.slice(1);if(data.pages.some(p=>p.id===hash))page=hash;render();
})();
