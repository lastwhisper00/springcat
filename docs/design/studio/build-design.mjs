import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const base = path.dirname(fileURLToPath(import.meta.url));
const tokens = JSON.parse(fs.readFileSync(path.join(base, 'visual-tokens.json'), 'utf8'));
const spec = JSON.parse(fs.readFileSync(path.join(base, 'interaction-spec.json'), 'utf8'));
const output = path.join(base, 'artboards');
fs.mkdirSync(output, { recursive: true });
const esc = s => String(s ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&apos;'}[c]));
const palettes = Object.fromEntries(tokens.themes.map(t=>[t.id,{...t.tokens,raised:t.tokens.surfaceRaised}]));
const sourceLogos = Object.fromEntries(['codex','cursor','gemini-cli','marvis'].map(name => [name, fs.readFileSync(path.join(base, 'assets/tool-logos', `${name}.svg`), 'utf8').replace(/^[\s\S]*?<svg[^>]*>/,'').replace(/<\/svg>[\s\S]*$/,'')]));
let T = palettes.graphite;
const rect = (x,y,w,h,r=0,fill=T.surface,stroke='none') => {
  const radius=(w===h&&r>=w/2)||(h<=48&&r>=h/2)?r:Math.min(r,w>=300&&h>=160?18:8);
  return `<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="${radius}" fill="${fill}" stroke="${stroke}"/>`;
};
const text = (x,y,s,size=14,fill=T.text,weight=400,anchor='start') => `<text x="${x}" y="${y}" font-size="${size}" font-weight="${weight}" fill="${fill}" text-anchor="${anchor}">${esc(s)}</text>`;
const line = (x,y,x2,y2,color=T.border) => `<path d="M${x} ${y}H${x2}V${y2}" stroke="${color}" fill="none"/>`;
const circle = (x,y,r,fill) => `<circle cx="${x}" cy="${y}" r="${r}" fill="${fill}"/>`;
function cat(x,y,size=22,color=T.text){return `<g transform="translate(${x} ${y}) scale(${size/32})" aria-label="SpringCat 状态标识"><path d="M6 13V26M16 5V26M26 10V26" stroke="${color}" stroke-width="3" fill="none"/></g>`;}
function activityOrb(x,y,size=28,running=0,source='Cursor',others=0){
  const cx=x+size/2,cy=y+size/2,logo=size<=22?13:16;
  return `<g aria-label="${esc(source)} · ${running>0?`${running} 项进行中`:'暂无进行中'}${others>0?` · 另有 ${others} 个运行来源`:''}">${running>0?`<circle cx="${cx}" cy="${cy}" r="${size/2+1}" fill="none" stroke="${T.working}" stroke-width="3" opacity=".35" style="filter:blur(3px)"/>`:''}${circle(cx,cy,size/2,T.raised)}<circle cx="${cx}" cy="${cy}" r="${size/2}" fill="none" stroke="${running>0?T.working:T.border}" stroke-width="1"/>${sourceIcon(x+(size-logo)/2,y+(size-logo)/2,source,logo,T.text)}${others>0?rect(x+size-4,y-5,20,14,7,T.raised)+text(x+size+6,y+5,`+${others}`,9,T.text,500,'middle'):''}</g>`;
}
function sourceIcon(x,y,source,size=20,color=T.text){const key=({'Codex':'codex','Cursor':'cursor','Gemini':'gemini-cli','Gemini CLI':'gemini-cli','Marvis':'marvis'})[source];return `<g transform="translate(${x} ${y}) scale(${size/(key==='marvis'?40:24)})" fill="${color}" color="${color}" fill-rule="evenodd" aria-label="${esc(source)}">${sourceLogos[key]||'<path d="M5 5h14v14H5z" fill="none" stroke="currentColor" stroke-width="1.5"/>'}</g>`;}
function icon(x,y,name,size=16,color=T.muted){const paths={expand:'M9 3H3v6M15 3h6v6M3 15v6h6M21 15v6h-6',collapse:'M6 12h12',arrow:'M6 18 18 6M6 6h12v12',chevron:'m9 5 7 7-7 7',check:'m5 12 4 4L19 6'};return `<g transform="translate(${x} ${y}) scale(${size/24})" stroke="${color}" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" fill="none"><path d="${paths[name]}"/></g>`;}
const label = (x,y,s,width=80,color=T.muted) => rect(x,y,width,24,4,T.raised)+text(x+width/2,y+16,s,11,color,500,'middle');
// Visible button surface is 28 pt; the desktop hit target extends to at least 32 pt.
const button = (x,y,w,s,primary=false) => rect(x,y,w,28,6,T.raised)+text(x+w/2,y+19,s,13,primary?T.accent:T.text,500,'middle');
const toggle = (x,y,on=true) => rect(x,y,34,20,10,on?T.accent:T.border)+circle(x+(on?24:10),y+10,7,on?T.surface:T.muted);
const desc = (x,y,lines,size=12,color=T.muted,gap=20) => lines.map((s,i)=>text(x,y+i*gap,s,size,color)).join('');
const stateText = {idle:'安静待命',running:'进行中',many:'3 个任务进行中',waiting:'待确认',completed:'已完成',merged:'3 个任务已完成',failed:'失败',cancelled:'已取消',muted:'静音中 · 58 分钟',focus:'专注中 · 2 条未读',loading:'正在同步状态',disconnected:'尚未连接工具'};
const colorFor = status => T[({running:'working',many:'working',waiting:'waiting',completed:'completed',merged:'completed',failed:'failed'})[status]] || T.muted;
function task(x,y,w,title,source,status,secondary='SpringCat 项目',hover=false){
  const pale=T.surface===palettes.porcelain.surface;
  return (hover?`<rect x="${x}" y="${y}" width="${w}" height="64" rx="8" fill="${pale?'#141C26':'#FFFFFF'}" fill-opacity="${pale?'.04':'.06'}"/>`:'')+sourceIcon(x+10,y+22,source,20,T.muted)+text(x+44,y+25,title,15,T.text,500)+text(x+44,y+47,`${source} · ${secondary}`,13,T.muted)+(status==='running'?`<circle cx="${x+w-61}" cy="${y+31}" r="3" fill="${T.working}" opacity=".22" style="filter:blur(1px)"/>`:'')+circle(x+w-61,y+31,2,colorFor(status))+text(x+w-10,y+36,stateText[status]||status,13,T.muted,400,'end');
}
function compact(x,y,w=248,h=40,status='waiting',notch=false,running=status==='many'?3:status==='running'?1:0){
  const size=h<=30?22:28,inset=h<=30?6:8;
  const summary=running>0?`${running} 项进行中`:'暂无进行中',sources=status==='many'?'Cursor、Codex':'Cursor';
  return rect(x,y,w,h,notch?12:h/2)+activityOrb(x+inset,y+(h-size)/2,size,running,'Cursor',status==='many'?1:0)+text(x+inset+size+(status==='many'?24:12),y+h/2+4,h<=30?summary:`${sources} · ${summary}`,12,T.text,500);
}
function panel(x,y,w=520,h=470){
  let s=rect(x,y,w,h,18,T.surface,T.border)+activityOrb(x+24,y+12,28,1,'Cursor')+text(x+64,y+32,'1 项进行中',13,T.muted,400);
  s+=text(x+w-236,y+34,'任务',13,T.text,500)+text(x+w-192,y+34,'动作',13,T.muted)+text(x+w-148,y+34,'回顾',13,T.muted)+icon(x+w-83,y+21,'expand')+icon(x+w-42,y+21,'collapse');
  s+=text(x+24,y+78,'最近任务',13,T.muted,500)+text(x+88,y+78,'5 项',13,T.muted)+text(x+w-24,y+78,'标记已读',13,T.muted,400,'end');
  const rows=[['重构设置页交互','Codex','waiting','刚刚'],['整理用户访谈','Cursor','running','1 分钟前'],['设计交接文档','Gemini CLI','waiting','2 分钟前'],['检查接口变更','Codex','waiting','4 分钟前'],['补充交互测试','Cursor','completed','10:18']];
  rows.forEach((row,i)=>{s+=task(x+24,y+101+i*64,w-48,...row)});
  s+=line(x+24,y+h-40,x+w-24,y+h-40)+circle(x+27,y+h-20,2,T.completed)+text(x+36,y+h-15,'已连接 · 本地',13,T.muted)+text(x+w-80,y+h-15,'全部任务',13,T.muted,400,'end')+icon(x+w-76,y+h-25,'chevron',12,T.muted)+text(x+w-24,y+h-15,'设置',13,T.muted,400,'end');
  return s;
}
function desktop(a){
  const w=a.width,h=a.height,side=a.place||'top',mode=a.mode||'summary';
  let s=rect(0,0,w,h,0,T.desktop)+rect(0,0,w,28,0,'#F2F4F6')+text(22,19,'工作文档     文件    编辑',11,'#4D5560')+text(w-20,19,'100%    周三 10:24',11,'#4D5560',400,'end');
  s+=rect(48,94,w-96,h-144,6,'#F8F9FB','#CCD2DA')+rect(48,94,142,h-144,6,'#EEF1F4')+text(66,121,'WORKSPACE',10,'#687481',500)+text(211,121,'release-checklist.md',12,'#4C5867',500)+line(48,139,w-48,139,'#DCE1E7');
  s+=desc(65,171,['springcat-ai','  src/','  docs/','  tests/','  package.json'],12,'#7B8490',30);
  s+=text(213,174,'发布检查',20,'#414B58',500)+desc(213,210,['01   检查任务状态与事件记录','02   验证来源应用跳转','03   核对通知与专注策略','04   检查窗口布局与系统缩放'],13,'#7B8490',31)+text(211,h-74,'示例工作区  ·  本地文件',11,'#8A949F');
  let pw=mode==='open'?520:mode==='closed'?(side==='notch'?170:side==='top'?128:36):248;
  let ph=mode==='open'?470:mode==='closed'?(side==='notch'?6:side==='top'?30:36):40;
  let px=side==='left'?10:side==='right'?w-pw-10:side==='free'?w-pw-80:(w-pw)/2;
  let py=side==='notch'?28:side==='left'||side==='right'?(mode==='open'?50:70):side==='free'?120:40;
  if(side==='notch')s+=rect(w/2-85,0,170,28,0,'#0D0E10');
  if(mode==='open')s+=panel(px,py,pw,ph);
  else if(mode==='closed'&&side==='notch')s+=rect(px,py,pw,ph,3,'#0D0E10')+circle(px+pw/2,py+3,2,T.working);
  else if(mode==='closed'&&side!=='top')s+=rect(px,py,36,36,18)+activityOrb(px+4,py+4,28,1,'Cursor');
  else s+=compact(px,py,pw,ph,mode==='closed'?'running':'waiting',side==='notch',1);
  if(side==='notch'&&mode!=='closed'){
    s+=rect(w/2-85,28,170,8,0,'#0D0E10');
  }
  if(a.ghost)s+=`<rect x="${w/2-130}" y="32" width="260" height="48" rx="20" fill="none" stroke="#5B7F9C" stroke-width="2" stroke-dasharray="5 5"/>`+text(w/2,103,'松开后吸附到顶部',13,'#435E76',500,'middle');
  s+=text(20,h-18,side==='notch'?'刘海尺寸由当前屏幕安全区域决定 · 图中仅为示意':`${pw} × ${ph} 逻辑尺寸 · ${side==='top'?'顶部中心轴固定':side==='free'?'自由位置，可拖动到边缘吸附':'靠边锚点固定'}`,12,'#536171');
  return s;
}
function statusBoard(a){
  const st=a.status;let s=rect(0,0,440,250,0,T.desktop)+text(24,35,a.title,17,'#344250',500);
  s+=rect(24,65,42,42,21)+activityOrb(31,72,28,st==='many'?3:st==='running'?1:0,'Cursor',st==='many'?1:0);
  s+=compact(84,65,330,42,st);
  const note={idle:['没有待处理事项，保持小体积。','入口仍可点击，打开最近任务。'],running:['球内展示执行来源的真实 Logo，外环微亮。','当前来源仍在执行时，不自动轮播 Logo。'],many:['示例：3 个任务来自 Cursor 与 Codex。','+1 表示另一个运行来源，不是另一项任务。'],waiting:['仅有待确认任务时，来源 Logo 保留。','执行外环熄灭；任务仍各占一行。'],completed:['短暂提示 5 秒，然后回到小体积。','保留未读标记，直到用户查看。'],merged:['连续完成合并为一条，避免连环打扰。','展开后可逐条查看。'],failed:['失败说明与恢复入口可见。','监听器不替来源工具自动重试。'],cancelled:['列表保留“已取消”，不用失败色。','没有其他事项时恢复待命。'],muted:['一小时内不自动探出，保留未读。','可手动恢复，计时到期恢复。'],focus:['提案：新事件只更新标记。','手动打开后仍可查看全部待处理。'],loading:['同步状态时保留最后可信信息。','未取得数据时不显示“0 个任务”。'],disconnected:['明确尚未连接工具。','唯一主操作：连接第一个工具。']}[st];
  s+=desc(26,157,note,13,'#536171',25);
  s+=text(26,222,st==='cancelled'?'仅列表状态；不增加独立自动弹出形态':'颜色 + 文案同时表达状态',11,'#637284');return s;
}
function appWindow(w,h,title,nav='动态'){
  let s=rect(0,0,w,h,16,T.surface,T.border)+rect(0,0,165,h,16,T.raised)+rect(145,0,20,h,0,T.raised);
  s+=circle(19,21,4,'#D7867E')+circle(33,21,4,'#D6B578')+circle(47,21,4,'#85B69A')+cat(22,51,24)+text(58,70,'SpringCat',15,T.text,500);
  ['动态','任务历史','用量与回顾','AI 工具','外观与停靠','通知与专注','常规与存储'].forEach((item,i)=>{let y=104+i*40;if(item===nav)s+=rect(12,y-20,140,33,8,T.surface);s+=text(24,y+1,item,12,item===nav?T.text:T.muted,item===nav?500:400);});
  s+=text(24,h-27,'本地优先 · 设计提案',11,T.muted)+text(196,59,title,25,T.text,500)+text(w-30,29,'−    ×',13,T.muted,400,'end');return s;
}
function workspace(a){
 let s=appWindow(860,560,a.title,a.kind==='history'?'任务历史':a.kind==='future'?'动态':'动态');
 const x=196,y=92,w=634;
 if(a.kind==='empty')return s+text(x,y+35,'暂无任务记录',22,T.text,500)+desc(x,y+70,['工具已连接。','来源应用产生任务后，状态会同步到此处。'],14)+button(x,y+138,160,'查看已连接工具');
 if(a.kind==='loading')return s+text(x,y+25,'正在读取任务…',14,T.muted)+[0,1,2].map(i=>rect(x,y+52+i*91,w,70,13,T.raised)+rect(x+16,y+68+i*91,210,10,5,T.border)+rect(x+16,y+90+i*91,130,8,4,T.border)).join('');
 if(a.kind==='error')return s+rect(x,y,w,300,18,T.raised)+text(x+26,y+50,'没有成功定位到原对话',22,T.text,500)+desc(x+26,y+86,['当前来源暂不支持精确跳转，或对应应用未安装。','任务仍保留未读；你可以先打开来源应用。'],14)+button(x+26,y+147,145,'打开来源应用',true)+button(x+182,y+147,120,'复制任务标题')+button(x+26,y+205,100,'重试跳转')+text(x+26,y+263,'新增反馈设计 · 原生能力需实现后验证',12,T.muted);
 if(a.kind==='detail')return s+text(x,y+10,'‹ 返回动态',12,T.muted)+label(x,y+33,'等待确认',88,T.waiting)+text(x,y+99,'重构设置页',27,T.text,500)+text(x,y+127,'Codex · SpringCat 项目 · 14:28',13,T.muted)+rect(x,y+157,w,125,14,T.raised)+desc(x+22,y+189,['已完成界面结构整理。','下一步需要你回到 Codex 查看具体请求。','SpringCat 仅展示任务状态与有限摘要。'],14,T.text,27)+button(x,y+309,148,'返回原对话',true)+button(x+162,y+309,100,'标为已读')+text(x,y+379,'摘要不包含完整对话、推理或工具参数。',12,T.muted);
 if(a.kind==='failed-detail')return s+label(x,y,'执行失败',86,T.failed)+text(x,y+63,'测试没有通过',26,T.text,500)+text(x,y+92,'Cursor · Web 应用 · 3 分钟前',13,T.muted)+rect(x,y+123,w,132,14,T.raised)+desc(x+22,y+157,['来源工具报告任务失败。','查看原始错误后，在来源工具决定是否重试。','这里不把来源应用操作伪装成已执行。'],14,T.text,28)+button(x,y+284,144,'查看原始错误',true)+button(x+158,y+284,110,'复制任务标题');
 if(a.kind==='future')return s+label(x,y,'未来概念',86,T.waiting)+text(x,y+73,'快捷操作',25,T.text,500)+['快速记录','打开工作区','整理对话','运行个人工作流'].map((v,i)=>{let by=y+106+i*65;return text(x+3,by+18,v,15,T.text,500)+text(x+310,by+18,'概念入口 · 尚未实现',12,T.muted)+text(x+w-10,by+18,'↗',14,T.muted,400,'end')+line(x,by+40,x+w,by+40)}).join('')+text(x,y+405,'后续能力共享展开空间，常驻胶囊保持小体积。',13,T.muted);
 if(a.kind==='history')s+=rect(x,y,w,38,9,T.raised)+text(x+14,y+25,'搜索任务标题、来源或项目',13,T.muted)+label(x,y+52,'全部',65,T.text)+label(x+75,y+52,'待处理',72)+label(x+157,y+52,'已完成',72)+text(x,y+111,'今天 · 9 月 16 日',13,T.muted);
 else s+=text(x,y+7,'最近任务',16,T.text,500)+text(x+w,y+7,'全部标为已读',12,T.muted,400,'end');
 const start=a.kind==='history'?y+136:y+33;
 const rows=[['重构设置页交互','Codex','waiting'],['整理用户访谈','Cursor','running'],['设计交接文档','Gemini CLI','waiting'],['检查接口变更','Codex','waiting'],['补充交互测试','Cursor','completed']];
 rows.forEach((r,i)=>{let ry=start+i*64;s+=task(x+5,ry,w-14,...r);if(i<4)s+=line(x,ry+63,x+w,ry+63)});
 return s;
}
function settingRow(x,y,w,title,sub,value,on){return text(x,y,title,14,T.text,500)+text(x,y+23,sub,12,T.muted)+(typeof on==='boolean'?toggle(x+w-35,y-11,on):text(x+w,y+9,value||'',13,T.muted,400,'end'))+line(x,y+45,x+w,y+45);}
function settings(a){
 const nav=({tools:'AI 工具',tool:'AI 工具',connecting:'AI 工具',repair:'AI 工具',passive:'AI 工具',appearance:'外观与停靠',notification:'通知与专注',storage:'常规与存储',restart:'常规与存储',general:'常规与存储',saveerror:'常规与存储'})[a.kind];
 let s=appWindow(860,620,a.title,nav),x=196,w=634,y=99;
 if(a.kind==='tools'){
  s+=text(x,y,'连接状态保存在本机，随时可停用。',13,T.muted);
  ['Codex','Cursor','ZCode','Grok CLI','Gemini CLI','WorkBuddy','Marvis','DSH Desktop'].forEach((name,i)=>{let yy=137+i*54;s+=rect(x,yy-19,30,30,8,T.raised)+text(x+15,yy+1,name.slice(0,2),11,T.text,500,'middle')+text(x+45,yy,name,14,T.text,500)+text(x+w-55,yy,i===2?'待检测':i>=5?'本地记录监听':'已绑定',12,i===2?T.waiting:T.completed,400,'end')+text(x+w-8,yy,'›',17,T.muted,400,'end')+line(x,yy+25,x+w,yy+25)});return s;
 }
 if(['tool','connecting','repair','passive'].includes(a.kind)){
  const passive=a.kind==='passive';s+=text(x,y+3,'‹ AI 工具',12,T.muted)+text(x,y+61,passive?'WorkBuddy':'Codex',27,T.text,500)+label(x+160,y+41,a.kind==='repair'?'需要修复':a.kind==='connecting'?'连接中':passive?'已检测到':'已连接',90,a.kind==='repair'?T.failed:a.kind==='connecting'?T.working:T.completed);
  s+=rect(x,y+97,w,148,14,T.raised)+text(x+20,y+129,passive?'只读监听本地会话记录':'监听通道',16,T.text,500)+desc(x+20,y+157,passive?['无需安装 hooks。检测到本地记录后自动显示任务。','尚无记录时显示“等待来源应用产生记录”。']:['本地会话记录 + hooks','开启监听仅更新 SpringCat 的配置项。'],13,T.muted,25);
  if(a.kind==='repair')s+=text(x+20,y+215,'检测未通过：请重新检测或修复安装。',13,T.failed);
  if(a.kind==='connecting')s+=text(x+20,y+215,'正在检查本地配置…',13,T.working);
  s+=button(x,y+266,136,a.kind==='connecting'?'正在连接…':a.kind==='repair'?'修复安装':passive?'重新检测':'测试连接',a.kind!=='connecting')+button(x+149,y+266,112,a.kind==='connecting'?'取消连接':passive?'暂停监听':'重新检测');
  if(!passive&&a.kind!=='connecting')s+=button(x,y+318,136,'复制手动配置')+button(x+149,y+318,112,'移除监听');
  s+=text(x,y+399,passive?'本地数据读取 · 不保存完整对话':'高级诊断  ›  路径、日志与手动配置',12,T.muted)+text(x,y+440,a.kind==='connecting'?'处理中禁止重复提交；取消后保留原连接。':'重启会话要求、信任提示按工具实际情况显示。',12,T.muted);return s;
 }
 if(a.kind==='appearance'){
  s+=text(x,y,'主题',15,T.text,500);['深墨','纸白','深海灰'].forEach((v,i)=>{let xx=x+i*213;s+=rect(xx,y+18,197,90,6,[palettes.graphite.surface,palettes.porcelain.surface,palettes.companion.surface][i],T.border)+text(xx+16,y+81,v,12,[palettes.graphite.text,palettes.porcelain.text,palettes.companion.text][i])+rect(xx+14,y+30,85,21,4,[palettes.graphite.raised,palettes.porcelain.raised,palettes.companion.raised][i])});
  s+=settingRow(x,266,w,'停靠位置','顶部优先；支持左右侧边与拖动。','顶部居中  ›')+settingRow(x,338,w,'适配 Mac 刘海','检测当前屏幕可显示区域。','',true)+settingRow(x,410,w,'减少动态效果','关闭空间运动，保留状态反馈。','',false)+settingRow(x,482,w,'显示密度','尺寸跟随系统缩放，不缩小主要命中区域。','标准  ›')+text(x,585,'主题、动态效果与密度为新增设计。',12,T.muted);return s;
 }
 if(a.kind==='notification'){
  s+=settingRow(x,119,w,'专注模式','提案：只更新未读标记，不自动展开。','',true)+settingRow(x,195,w,'静音 1 小时','所有提醒保持安静；手动打开不受影响。','剩余 58 分钟')+settingRow(x,271,w,'完成提醒','合并连续完成，5 秒后回到小体积。','短暂提示  ›')+settingRow(x,347,w,'等待与失败','保留待处理标记，遵守用户手动收起。','持续标记  ›')+settingRow(x,423,w,'任务执行时自动置顶','没有执行任务时恢复原先状态。','',false)+text(x,531,'专注策略拟调整；当前版本仅抑制完成时探出。',12,T.waiting);return s;
 }
 if(a.kind==='storage'||a.kind==='restart'){
  s+=settingRow(x,119,w,'历史保留','只保存生命周期元数据与有限摘要。','7 天  ›')+settingRow(x,195,w,'本地存储目录','任务缓存、事件文件与日志均保存在本机。','更改目录  ›')+settingRow(x,271,w,'外部链接浏览器','打开 HTTP(S) 链接时使用。','跟随系统  ›')+rect(x,328,w,114,14,T.raised)+desc(x+20,358,['不保存完整对话、推理、工具参数或项目代码。','用量读取结构化 Token 字段。','更换存储目录需要复制历史并重启。'],13,T.muted,26);
  if(a.kind==='restart')s+=rect(x,464,w,103,13,T.raised)+text(x+18,495,'数据复制完成，重启后使用新目录',15,T.text,500)+button(x+18,514,110,'立即重启',true)+button(x+141,514,100,'稍后重启');
  return s;
 }
 s+=settingRow(x,119,w,'开机启动','登录系统后显示 SpringCat。','',true)+settingRow(x,195,w,'保持置顶','让常驻入口停留在其他窗口之上。','',true)+settingRow(x,271,w,'双击入口','打开最近的待处理任务。','打开最近任务  ›')+settingRow(x,347,w,'默认停靠','重新启动后使用此位置。','顶部居中  ›')+settingRow(x,423,w,'展示模式','使用统一的任务与状态界面。','标准模式  ›');
 s+=a.kind==='saveerror'?rect(x,500,w,65,12,T.raised)+text(x+17,525,'设置未能保存，已保留上次有效设置。',13,T.failed)+text(x+17,547,'重试保存',12,T.text,500):text(x,546,'设置自动保存 · 已保存',12,T.completed);return s;
}
function onboarding(a){
 let s=rect(0,0,720,510,18,T.surface,T.border)+cat(38,32,32)+text(83,56,'SpringCat',19,T.text,500)+text(677,53,'跳过',12,T.muted,400,'end');
 ['认识 SpringCat','选择位置','连接工具','测试提醒'].forEach((v,i)=>{s+=circle(55+i*174,101,11,i===a.step?T.accent:T.raised)+text(55+i*174,105,i+1,11,i===a.step?T.surface:T.muted,500,'middle')+text(75+i*174,106,v,12,i===a.step?T.text:T.muted)});
 if(a.step===0)s+=text(48,191,'统一查看 AI 任务状态',28,T.text,500)+desc(48,239,['连接常用的 AI 工具。','集中查看运行、待确认与异常状态。'],15,T.muted,30)+compact(48,321,265,42,'waiting');
 if(a.step===1){s+=text(48,178,'选择一个舒服的位置',25,T.text,500);['顶部居中','Mac 刘海下方','屏幕侧边'].forEach((v,i)=>{let xx=48+i*214;s+=rect(xx,207,196,144,14,T.raised)+rect(xx+14,224,168,88,7,T.desktop)+rect(xx+(i===2?154:65),i===1?224:230,i===2?17:68,i===1?12:18,9)+text(xx+18,337,v,13,T.text,500)});}
 if(a.step===2)s+=text(48,178,'先连接一个常用工具',25,T.text,500)+['Codex','Cursor','Gemini CLI'].map((v,i)=>rect(48,201+i*54,622,44,11,T.raised)+text(67,228+i*54,v,14,T.text,500)+text(649,228+i*54,i===0?'已选择':'连接',12,i===0?T.completed:T.muted,400,'end')).join('')+text(48,396,'你可以稍后在设置中连接全部 8 个来源。',13,T.muted);
 if(a.step===3)s+=text(48,179,'体验一次任务提醒',25,T.text,500)+compact(48,217,315,42,'completed')+text(48,302,'这是一条测试提醒，不会创建真实 AI 任务。',14,T.muted)+button(48,335,132,'再试一次');
 s+=line(38,437,682,437)+text(48,473,'首次使用向导 · 新增设计',12,T.muted)+button(559,450,112,a.step===3?'开始使用':'继续',true);return s;
}
function usage(a){
 let s=appWindow(900,640,a.title,'用量与回顾'),x=195,w=675;
 s+=label(x,89,'日',52,a.period==='day'?T.text:T.muted)+label(x+62,89,'周',52,a.period==='week'?T.text:T.muted)+label(x+124,89,'月',52,a.period==='month'?T.text:T.muted)+text(x+w,106,a.period==='day'?'‹  9 月 16 日  ›':a.period==='week'?'‹  9 月 14–20 日  ›':'‹  2026 年 9 月  ›',14,T.muted,400,'end');
 if(a.kind==='usage-empty')return s+text(x,203,'这个周期还没有用量记录',25,T.text,500)+desc(x,244,['任务监听与用量统计的来源覆盖不同。','连接支持统计的工具，并等待本地用量记录产生。'],14)+button(x,315,176,'检查支持统计的工具');
 if(a.kind==='usage-error')return s+rect(x,155,w,177,14,T.raised)+text(x+22,196,'暂时无法读取用量',24,T.text,500)+text(x+22,230,'保留当前日期范围，重新读取即可继续。',14,T.muted)+button(x+22,264,115,'重新读取',true);
 if(a.kind==='share')return s+rect(x+75,137,490,390,19,T.raised)+text(x+101,177,'分享用量图',21,T.text,500)+rect(x+102,197,436,221,12,T.surface)+text(x+126,231,'SpringCat / 9 月 16 日',14,T.muted)+text(x+126,290,'168,400',39,T.text,500)+text(x+126,318,'Token · 示例数据',13,T.muted)+text(x+126,369,'不包含提示词与对话内容',12,T.muted)+button(x+102,441,135,'复制图片',true)+button(x+250,441,135,'保存 PNG')+text(x+102,506,'生成中 / 复制成功 / 保存失败分别反馈',11,T.muted);
 if(a.kind==='calendar'){
   s+=text(x,162,'2026 年 9 月',23,T.text,500)+text(x+380,160,'点击日期查看当日用量',12,T.muted);
   ['一','二','三','四','五','六','日'].forEach((d,i)=>s+=text(x+26+i*58,208,d,12,T.muted,400,'middle'));
   for(let day=1;day<=30;day++){const col=day%7,row=Math.floor(day/7),xx=x+col*58,yy=225+row*55;s+=rect(xx,yy,49,44,8,day===16?T.accent:day<16?T.raised:T.surface)+text(xx+24,yy+20,day,12,day===16?T.surface:T.text,500,'middle');if(day<=16)s+=circle(xx+24,yy+33,2,day===16?T.surface:T.completed);}
   s+=rect(x+440,208,229,210,14,T.raised)+text(x+460,242,'9 月 16 日',17,T.text,500)+text(x+460,292,'168,400',29,T.text,500)+text(x+460,319,'Token · 示例',12,T.muted)+button(x+460,350,174,'查看当日明细',true)+text(x,556,'有记录日期用静态标记；无记录与未来日期不编造数值。',12,T.muted);return s;
 }
 if(a.kind==='breakdown'){
  s+=text(x,162,'Token 构成',19,T.text,500);
  const metrics=[['输入（含缓存）','123,400'],['其中缓存','84,200'],['输出（含推理）','45,000'],['其中推理','12,500']];
  metrics.forEach((m,i)=>{let xx=x+(i%2)*340,yy=186+Math.floor(i/2)*94;s+=rect(xx,yy,322,79,13,T.raised)+text(xx+18,yy+28,m[0],12,T.muted)+text(xx+18,yy+60,m[1],24,T.text,500)});
  s+=text(x,395,'模型明细',17,T.text,500);[['模型 A','Codex','70,000'],['模型 B','Codex','32,300'],['模型 C','Grok CLI','66,100']].forEach((r,i)=>{let yy=431+i*41;s+=text(x,yy,r[0],13,T.text)+text(x+240,yy,r[1],12,T.muted)+text(x+w,yy,r[2],13,T.text,400,'end')+line(x,yy+15,x+w,yy+15)});
  s+=text(x,587,'合计 168,400 Token；缓存和推理是子项，不能重复相加。',12,T.muted);return s;
 }
 s+=text(x,158,'总 Token',13,T.muted)+text(x,207,a.period==='day'?'168,400':a.period==='week'?'892,600':'2,184,300',41,T.text,500)+text(x+310,158,'API 等价估算',13,T.muted)+text(x+310,201,a.period==='day'?'$ 1.28':a.period==='week'?'$ 6.94':'$ 17.86',29,T.text,500)+text(x+310,226,'估算不等于订阅实际账单',12,T.muted);
 const monthRaw=[88.2,112.8,155.4,138.6,124.3,167.6,121.4,135.2,168.4,140.5,170.1,112.2,126.8,154.6,99.8];
 const dayValues=[6800,10200,14200,16800,25400,30200,26800,18100,11700,8200];
 const weekValues=[148800,160500,168400,120800,97200,106000,90900];
 const monthTotal=2184300, remaining=monthTotal-168400, factor=remaining/monthRaw.reduce((sum,n)=>sum+n,0);
 const monthValues=monthRaw.map(n=>Math.round(n*factor));monthValues[14]+=remaining-monthValues.reduce((sum,n)=>sum+n,0);monthValues.push(168400);
 const values=a.period==='day'?dayValues:a.period==='week'?weekValues:monthValues;
 const ceiling=a.period==='day'?40000:200000;
 s+=text(x,266,a.period==='day'?'每小时用量 · 千 Token':'每日用量 · 千 Token',14,T.text,500);
 s+=line(x+30,397,x+w,397)+text(x,299,String(ceiling/1000),10,T.muted)+text(x+7,350,String(ceiling/2000),10,T.muted)+text(x+14,397,'0',10,T.muted);
 values.forEach((v,i)=>{let bx=x+47+i*((w-56)/values.length),height=v/ceiling*110;s+=rect(bx,397-height,(w-90)/values.length-7,height,4,T.accent)+text(bx+8,418,a.period==='day'?String(8+i):a.period==='week'?String(14+i):String(i+1),10,T.muted)});
 s+=text(x+w,438,a.period==='day'?'小时':'日期',11,T.muted,400,'end')+line(x,452,x+w,452)+text(x,479,'来源与模型',15,T.text,500)+text(x+w,479,'查看明细  ›',12,T.muted,400,'end');
 const sources=a.period==='day'?['102,300','66,100']:a.period==='week'?['541,900','350,700']:['1,326,300','858,000'];
 [['Codex','模型 A · 示例',sources[0]],['Grok CLI','模型 B · 示例',sources[1]]].forEach((r,i)=>{let yy=512+i*43;s+=text(x,yy,r[0],13,T.text,500)+text(x+140,yy,r[1],12,T.muted)+text(x+w,yy,r[2],13,T.text,400,'end')});
 s+=text(x,612,'全部数值仅用于设计展示 · 来源 / 时间范围清晰标注',11,T.muted);return s;
}
function components(a){
 let s=rect(0,0,800,500,0,T.desktop)+text(30,41,a.title,22,'#344250',500);
 if(a.kind==='themes'){
  let original=T;Object.entries(palettes).forEach(([key,p],i)=>{T=p;let xx=28+i*257;s+=compact(xx,78,230,40,'waiting',false,1)+rect(xx,142,230,279,16,T.surface,T.border)+activityOrb(xx+18,150,28,1,'Cursor')+text(xx+58,171,'1 项进行中',13,T.muted)+text(xx+18,215,'最近任务',13,T.muted,500)+text(xx+212,215,'3 项',13,T.muted,400,'end');
    [['设置页交互','Codex','waiting','刚刚'],['用户访谈','Cursor','running','1 分钟前'],['交接文档','Gemini CLI','waiting','2 分钟前']].forEach((row,n)=>{s+=task(xx+18,228+n*64,194,...row)});
    s+=text(xx,448,['深墨 / 主推荐','纸白 / 浅色','深海灰 / 蓝灰'][i],12,'#536171')});T=original;
  return s+text(30,481,'所有任务使用同一行结构；状态文字与小色点表达差异。',13,'#536171');
 }
 if(a.kind==='tokens'){
  const colors=['surface','raised','text','muted','accent','working','waiting','completed','failed'];colors.forEach((name,i)=>{let xx=30+(i%5)*151,yy=83+Math.floor(i/5)*141;s+=rect(xx,yy,126,71,12,T[name],T.border)+text(xx,yy+96,name,12,'#425262',500)+text(xx,yy+116,T[name],11,'#667381')});return s+desc(30,414,['颜色始终搭配状态文案；正文与按钮采用足够对比度。','Mac 刘海外壳保持黑色融合，内容区域遵循所选主题。'],13,'#536171',24);
 }
 if(a.kind==='type')return s+rect(28,76,744,372,16,T.surface)+text(52,112,'Noto Sans SC · 自托管可变字体',13,T.muted)+text(52,150,'任务详情标题 / 20 · 500',20,T.text,500)+text(52,193,'默认',13,T.muted)+task(142,166,596,'重构设置页交互','Codex','waiting','刚刚')+text(52,257,'悬停',13,T.muted)+task(142,230,596,'重构设置页交互','Codex','waiting','刚刚',true)+line(52,314,746,314)+desc(52,347,['任务标题 15 / 500 / 22；辅助与状态 13 / 400 / 20。','64 pt 行高；两行间距 2，四边内距 10；字号不按状态变化。','悬停只改变整行半透明背景，文字、图标和状态颜色保持不变。','背景过渡 120 ms；减少动态效果时即时切换。'],13,T.muted,25);
 if(a.kind==='fields')return s+rect(28,76,744,372,16,T.surface)+text(50,111,'输入与特殊状态',16,T.text,500)+text(50,148,'默认',12,T.muted)+rect(50,160,323,37,9,T.raised,T.border)+text(65,184,'搜索任务标题',13,T.muted)+text(419,148,'键盘聚焦',12,T.muted)+rect(415,157,331,43,11,'none',T.accent)+rect(419,161,323,35,8,T.raised)+text(434,184,'SpringCat',13,T.text)+text(50,234,'校验错误',12,T.muted)+rect(50,247,323,37,9,T.raised,T.failed)+text(65,271,'当前目录无法写入',13,T.failed)+text(50,310,'请更换目录或检查写入权限。',11,T.failed)+text(419,234,'只读',12,T.muted)+rect(419,247,323,37,9,T.raised,T.border)+text(434,271,'由来源工具提供 · 不可编辑',12,T.muted)+rect(50,350,133,32,9,T.border)+text(116,372,'按钮按下',12,T.text,500,'middle')+text(213,371,'只读任务仅提供“返回原对话”，不显示伪授权按钮。',12,T.muted);
 if(a.kind==='feedback'){
  const rows=[['生成中','正在生成用量图片…','请稍候，可取消'],['复制成功','图片已复制','可粘贴到目标应用'],['复制失败','系统未能复制图片','改为保存 PNG'],['保存成功','已保存到所选目录','打开所在目录'],['保存失败','当前目录不可写入','重新选择目录'],['连接测试成功','已收到测试事件','连接可以使用']];
  rows.forEach((r,i)=>{let xx=28+(i%2)*386,yy=84+Math.floor(i/2)*120;s+=rect(xx,yy,358,99,12,T.surface)+circle(xx+20,yy+25,3,i===2||i===4?T.failed:T.completed)+text(xx+32,yy+29,r[0],13,T.text,500)+text(xx+16,yy+56,r[1],12,T.muted)+text(xx+16,yy+80,r[2],11,T.accent)});
  return s+text(28,478,'反馈紧邻触发动作；失败保留原内容与恢复入口。',12,'#536171');
 }
 if(a.kind==='controls'){
  s+=rect(28,76,744,372,16,T.surface)+text(50,112,'轻量操作',14,T.text,500)+text(50,150,'查看请求',13,T.accent,500)+icon(111,138,'arrow',14,T.accent)+button(155,131,106,'返回对话',true)+button(273,131,106,'重新检测')+button(391,131,106,'处理中…')+rect(509,131,98,28,6,T.raised)+text(558,150,'不可用',13,T.muted,400,'middle')+rect(617,127,123,36,8,'none',T.accent)+button(621,131,115,'键盘焦点');
  s+=text(50,190,'文字 13 / 500 · 可见按钮高 28 · 命中区域至少 32 · 无大面积强调色',12,T.muted);
  s+=text(50,231,'状态与来源',14,T.text,500)+['running','waiting','completed','failed','cancelled'].map((st,i)=>circle(53+i*139,258,2.5,colorFor(st))+text(63+i*139,262,stateText[st],12,colorFor(st))).join('');
  s+=text(50,310,'开关 / 选择 / 悬停',14,T.text,500)+toggle(50,330,true)+toggle(107,330,false)+icon(166,330,'check',16,T.text)+text(190,343,'已选中',13,T.text,500)+rect(278,324,124,28,6,T.raised)+text(340,343,'悬停高亮',13,T.text,500,'middle')+sourceIcon(439,329,'Codex',20,T.muted)+sourceIcon(479,329,'Cursor',20,T.muted)+sourceIcon(519,329,'Gemini CLI',20,T.muted)+text(50,410,'每处只突出下一步；禁用说明原因，等待与失败保留恢复入口。',12,T.muted);return s;
 }
 if(a.kind==='menu')return s+compact(58,76,248,40)+rect(58,126,284,305,14,T.surface,T.border)+['查看所有任务','全部标为已读','静音 1 小时','专注模式     ✓','置顶','设置…','退出 SpringCat'].map((v,i)=>text(78,156+i*40,v,13,i===6?T.muted:T.text)).join('')+desc(385,130,['入口右键 / 托盘菜单','查看、静音、置顶和设置。','退出与隐藏为两个不同动作。','当前使用原生菜单；外观随平台。','本图为菜单信息架构提案。'],14,'#536171',32);
 if(a.kind==='pet')return s+rect(28,83,744,343,8,T.surface)+activityOrb(53,112,28,1,'Cursor')+text(93,131,'1 项进行中',12,T.muted)+activityOrb(268,112,28,0,'Cursor')+text(308,131,'暂无进行中',12,T.muted)+activityOrb(483,112,28,3,'Cursor',1)+text(531,131,'3 项 · 2 个来源',12,T.muted)+text(53,176,'来源球 28 / Logo 16 · 中性实底 · 外环表示执行',12,T.muted)+line(53,203,748,203)+compact(53,236,128,30,'idle')+compact(210,231,248,40,'running')+text(53,305,'常驻状态',12,T.muted)+text(210,305,'来源与状态摘要',12,T.muted)+desc(53,351,['Logo 表示来源，外环表示执行；+N 是其他运行来源数。','当前来源仍在执行时保持不变；无执行时保留 Logo，熄灭外环。'],13,T.muted,25);
 return s;
}
function wrapCopy(value,limit=49){const lines=[];let row='',width=0;for(const ch of String(value)){const unit=/[\u0000-\u00ff]/.test(ch)?.56:1;if(width+unit>limit&&row){lines.push(row);row='';width=0}row+=ch;width+=unit}if(row)lines.push(row);return lines;}
function flow(a){
 const f=a.flow;let s=text(28,40,f.title,23,'#344250',500)+label(28,57,f.status==='current'?'已有能力':'交互提案',84);
 const trigger=wrapCopy(f.trigger,49);s+=text(28,114,'触发',12,'#697785',500)+desc(81,114,trigger,13,'#435364',20);let y=140+(trigger.length-1)*20;
 f.steps.slice(0,5).forEach((step,i)=>{const rows=wrapCopy(step,50),height=rows.length*20+16;s+=circle(40,y,10,T.surface)+text(40,y+4,i+1,11,T.text,500,'middle')+desc(62,y+4,rows,13,'#435364',20);if(i<f.steps.length-1)s+=line(40,y+12,40,y+height-12,'#BAC5CD');y+=height});
 y+=12;const close=wrapCopy(f.close,53);s+=text(28,y,'收束',12,'#697785',500)+desc(81,y,close,12,'#435364',19);y+=close.length*19+18;const note=wrapCopy(f.edgeCases?.[0]||'',61);s+=desc(28,y,note,11,'#667785',18);a.height=Math.max(405,y+note.length*18+22);return rect(0,0,760,a.height,0,T.desktop)+s;
}
const boards=[];
function add(page,id,title,kind,width,height,extra={}){boards.push({page,id,title,kind,width,height,availability:'界面改版',entry:'从对应入口打开',action:'按图中主要动作继续',exit:'返回上一级或收起',note:'示例内容；交互规则是待落地的设计规格。',...extra});}
const places=[['top','顶部居中'],['notch','Mac 刘海'],['left','左侧贴边'],['right','右侧贴边']];
for(const [p,name]of places)for(const [m,label0]of [['closed','静默'],['summary','摘要'],['open','展开']])add('placement',`${p}-${m}`,`${name} / ${label0}`,'desktop',800,560,{place:p,mode:m,availability:'交互提案',entry:m==='summary'?'悬停 180ms 或新的重要事件':m==='open'?'点击入口 / 键盘激活':'手动收起 / Escape',action:m==='open'?'五条同级任务；超过五条分页，完整列表进入工作台':'点击进入完整面板',exit:m==='summary'?'离开 220ms 后收起；持久提醒遵守通知规则':'原锚点收束，保留未读',note:p==='notch'?'硬件刘海不可绘制内容；实际宽高读取屏幕安全区域。展开层宽520，两侧留白24。':p==='top'?'常驻128×30；展开520宽，内容自然决定高度。顶部中心固定。':'常驻球保持36；展开宽520并向桌面内侧避让，五条同级任务同屏可见。'});
add('placement','free-drag','自由拖动与吸附预览','desktop',800,460,{place:'free',mode:'closed',ghost:true,availability:'交互提案',entry:'按住入口并拖动',action:'接近顶部或左右边缘显示吸附预览',exit:'松开确认；Escape 恢复拖动前位置',note:'预览不抢占其他窗口；跨屏后依据目标屏幕安全区域重算。'});
for(const st of Object.keys(stateText))add('states',`status-${st}`,stateText[st],'status',440,250,{status:st,availability:st==='focus'||st==='loading'?'交互提案':'已有能力 / 新样式',entry:'来源状态变化或用户操作',action:['waiting','failed'].includes(st)?'回到原对话处理':'主动展开查看',exit:st==='completed'||st==='merged'?'5 秒后收起并保留未读':st==='muted'?'一小时到期或手动恢复':'遵守当前模式与用户意图',note:'列表中所有状态同级；紧凑入口的状态汇总与列表排序分离。当前代码的紧凑主状态为运行优先。'});
for(const [id,title]of [['dynamic','任务动态'],['detail','任务详情 / 待确认'],['failed-detail','任务详情 / 失败'],['history','任务历史与筛选'],['empty','已连接但暂无任务'],['loading','任务读取中'],['error','无法打开原对话'],['future','未来动作入口']])add('workspace',id,title,'workspace',860,560,{kind:id,availability:id==='future'?'未来概念':['dynamic','empty'].includes(id)?'已有能力 / 新样式':'设计补齐',entry:id==='detail'?'点击待处理任务':'工作台对应入口',action:id==='future'?'概念入口，未连接实际执行能力':'主要动作优先，恢复路径可见',exit:'返回动态；关闭独立窗口不退出常驻入口',note:id==='error'?'打开结果未确认前保留未读；精确跳转能力因来源而异。':'较长内容使用独立工作窗口；常驻胶囊体积保持稳定。'});
for(const [id,title]of [['tools','AI 工具连接'],['tool','工具详情 / 已连接'],['connecting','工具详情 / 连接中'],['repair','工具详情 / 修复'],['passive','工具详情 / 只读监听'],['appearance','外观与停靠'],['notification','通知与专注'],['general','常规设置'],['storage','本地存储与隐私'],['restart','存储迁移 / 重启提示'],['saveerror','设置保存失败']])add('settings',`settings-${id}`,title,'settings',860,620,{kind:id,availability:['appearance','saveerror'].includes(id)?'设计补齐':'已有能力 / 新样式',entry:'设置对应分类',action:id==='connecting'?'等待；不重复提交':id==='repair'?'修复 / 手动配置 / 查看诊断':'修改后自动保存并反馈结果',exit:'返回分类；关闭设置不退出程序',note:'hook 工具和只读监听工具的配置路径不同；保留原有第三方配置。'});
for(let i=0;i<4;i++)add('onboarding',`onboard-${i}`,['欢迎与价值','选择停靠位置','连接第一个工具','测试提醒与完成'][i],'onboarding',720,510,{step:i,availability:'设计补齐',entry:i===0?'首次运行':'向导上一步',action:i===3?'试一次提醒后开始使用':'选择后继续；允许稍后设置',exit:'完成进入小体积常驻；设置中可重新调整',note:'当前版本没有首次使用向导，这组是新增设计。'});
for(const p of ['day','week','month'])add('usage',`usage-${p}`,({day:'用量 / 日',week:'用量 / 周',month:'用量 / 月'})[p],'usage',900,640,{period:p,availability:'已有能力 / 新样式',entry:'用量与回顾',action:'切周期、选日期、查看来源与模型',exit:'返回工作台',note:'所有图中数据为示例；估算不等于实际账单，来源覆盖有差异。'});
for(const [id,title]of [['usage-empty','用量 / 暂无记录'],['usage-error','用量 / 读取失败'],['share','用量图分享预览']])add('usage',id,title,'usage',900,640,{period:'day',kind:id,availability:'已有能力 / 新样式',entry:id==='share'?'分享用量图':'读取用量后',action:id==='share'?'预览后复制或保存图片':'检查来源或重试读取',exit:id==='share'?'关闭预览返回用量':'保留已选日期范围',note:'数据缺失不伪装为零用量。分享由用户主动操作。'});
add('usage','usage-calendar','月历与日期选择','usage',900,640,{kind:'calendar',period:'month',availability:'已有能力 / 新样式',entry:'选择日期 / 月历',action:'点击有记录日期切为当日明细',exit:'返回所选周期',note:'无记录日期与未来日期均不编造用量。'});
add('usage','usage-breakdown','输入、缓存、输出与模型明细','usage',900,640,{kind:'breakdown',period:'day',availability:'已有能力 / 新样式',entry:'用量 → 来源与模型明细',action:'查看构成、来源和模型',exit:'返回所选周期',note:'缓存是输入子项，推理是输出子项；所有示例数值内部一致。'});
for(const [id,title]of [['themes','三个主题 / 同一交互'],['tokens','颜色与状态语义'],['type','文字与尺寸'],['controls','按钮与组件状态'],['menu','右键与托盘菜单'],['pet','智能体来源球与多来源']])add('style',`style-${id}`,title,'components',800,500,{kind:id,availability:'设计规范',entry:'同一设计系统',action:'主题只改变外观，交互和常驻尺寸一致',exit:'沿用统一状态模型',note:'按下、悬停、焦点、禁用和错误分别可识别。'});
add('style','style-fields','字段与特殊状态','components',800,500,{kind:'fields',availability:'设计规范',entry:'输入或只读详情',action:'默认、聚焦、错误、只读与按下态区分清楚',exit:'修改有效后恢复；只读状态保持说明',note:'错误原因紧邻字段；禁用与只读使用不同语义。'});
add('style','style-feedback','处理中、成功与失败反馈','components',800,500,{kind:'feedback',availability:'已有能力 / 新样式',entry:'测试连接、复制或保存图片',action:'按结果反馈；失败时提供恢复动作',exit:'成功反馈淡出；失败保持到处理或关闭',note:'状态反馈不能掩盖结果，也不能把未完成的操作报为成功。'});
for(const f of spec.flows)add('flows',f.id,f.title,'flow',760,405,{flow:f,availability:f.status==='current'?'已有能力':'交互提案',entry:f.trigger,action:f.steps.join(' → '),exit:f.close,note:f.edgeCases.join('；')});
const pages=[
 {id:'placement',name:'停靠与形态',subtitle:'顶部、刘海、左右侧边与自由拖动。'},
 {id:'states',name:'任务与提醒状态',subtitle:'常规、异常、专注与静音的完整表达。'},
 {id:'workspace',name:'任务工作台',subtitle:'查看进展、处理任务、检索历史与未来动作。'},
 {id:'onboarding',name:'首次使用',subtitle:'从理解价值，到连接第一个工具。'},
 {id:'settings',name:'设置与工具连接',subtitle:'8 个来源接入，以及常驻行为与本地存储。'},
 {id:'usage',name:'用量与分享',subtitle:'日、周、月、缺失反馈与分享预览。'},
 {id:'style',name:'主题与组件',subtitle:'深墨、纸白、深海灰共享一套专业工具规范。'},
 {id:'flows',name:'交互流程',subtitle:'触发、状态变化、结束方式与边界处理。'}
];
function render(a,theme){T=palettes[theme];let content;switch(a.page){case'placement':content=desktop(a);break;case'states':content=statusBoard(a);break;case'workspace':content=workspace(a);break;case'onboarding':content=onboarding(a);break;case'settings':content=settings(a);break;case'usage':content=usage(a);break;case'style':content=components(a);break;case'flows':content=flow(a);break;}return `<svg xmlns="http://www.w3.org/2000/svg" width="${a.width}" height="${a.height}" viewBox="0 0 ${a.width} ${a.height}" role="img" aria-label="${esc(a.title)}"><title>${esc(a.id+' / '+a.title)}</title><desc>${esc(a.availability+'。'+a.note)}</desc><style>text{font-family:'SpringCat Sans','Noto Sans SC','PingFang SC','Microsoft YaHei',sans-serif}</style>${content}</svg>`;}
for(const a of boards){a.svgs={};for(const name of Object.keys(palettes)){a.svgs[name]=render(a,name);fs.writeFileSync(path.join(output,`${a.id}-${name}.svg`),a.svgs[name]);}}
const data={title:'SpringCat / Product Design System',version:'Concept 09 · Type',pages,boards,palettes,tokens,spec};
fs.writeFileSync(path.join(base,'design-data.js'),'window.SPRINGCAT_DESIGN = '+JSON.stringify(data)+';\n');
fs.writeFileSync(path.join(base,'screen-index.json'),JSON.stringify(boards.map(({svgs,...a})=>a),null,2));
const sheets=path.join(base,'sheets');fs.mkdirSync(sheets,{recursive:true});
for(const [pi,p]of pages.entries())for(const name of Object.keys(palettes)){
 const list=boards.filter(a=>a.page===p.id),cols=p.id==='states'?4:3,cellW=p.id==='states'?488:960,cellH=Math.max(...list.map(a=>a.height))+96,rows=Math.ceil(list.length/cols),width=cols*cellW+80,height=rows*cellH+145;
 let sheet=`<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}"><style>text{font-family:'SpringCat Sans','Noto Sans SC','PingFang SC','Microsoft YaHei',sans-serif}</style><rect width="${width}" height="${height}" fill="#EDF0F4"/><text x="40" y="53" font-size="27" fill="#293946">SpringCat / ${esc(p.name)} / ${esc(name)}</text><text x="40" y="82" font-size="14" fill="#677B8C">Concept 09 · Type · 常驻小体积 / 展开520宽 / Noto Sans SC / 舒适字号 / 整行悬停 · 示例内容 · SVG设计图</text>`;
 list.forEach((a,i)=>{const x=40+(i%cols)*cellW,y=127+Math.floor(i/cols)*cellH;sheet+=`<g id="${a.id}"><text x="${x}" y="${y}" font-size="16" fill="#415667">${esc(a.title)}</text><text x="${x}" y="${y+22}" font-size="11" fill="#7D8B9B">${esc(a.id+' · '+a.availability)}</text><g transform="translate(${x} ${y+34})">${a.svgs[name].replace(/^<svg[^>]*>/,'').replace(/<\/svg>$/,'')}</g></g>`});
 sheet+='</svg>';fs.writeFileSync(path.join(sheets,`${String(pi+1).padStart(2,'0')}-${p.id}-${name}.svg`),sheet);
}
console.log(JSON.stringify({boards:boards.length,pages:pages.length,themes:Object.keys(palettes).length,svgFiles:boards.length*3}));
