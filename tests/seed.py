import urllib.request,json,time,subprocess
import os
base=os.environ.get('MEILI_TEST_URL','http://127.0.0.1:17700');key=os.environ.get('MEILI_TEST_KEY','tabularis-meili-local-demo-key')
# Wait briefly for the dedicated CI/local container to become ready.
for attempt in range(60):
 try:
  with urllib.request.urlopen(base+'/health',timeout=2): pass
  break
 except Exception:
  if attempt == 59: raise
  time.sleep(.25)
def api(path,data=None,method=None):
 r=urllib.request.Request(base+path,data=None if data is None else json.dumps(data).encode(),headers={'Authorization':'Bearer '+key,'Content-Type':'application/json'},method=method)
 return json.load(urllib.request.urlopen(r))
def wait(task):
 for _ in range(100):
  t=api('/tasks/'+str(task['taskUid']))
  if t['status']=='succeeded':return t
  if t['status'] in ['failed','canceled']:raise AssertionError(t)
  time.sleep(.05)
 raise TimeoutError()
# Only reset the two explicitly named disposable demo indexes.
for name in ['movies','empty_index']:
 try:
  api('/indexes/'+name)
  wait(api('/indexes/'+name,method='DELETE'))
 except urllib.error.HTTPError as e:
  if e.code != 404: raise
wait(api('/indexes',{'uid':'movies','primaryKey':'id'}))
docs=[{'id':i,'title':('The Matrix' if i==1 else 'Movie '+str(i)),'year':1999 if i%2 else 2024,'genres':['Sci-Fi'] if i%2 else ['Drama'],'rating':8.5,'available':True,'details':{'language':'en'},'optional':None} for i in range(1,206)]
wait(api('/indexes/movies/documents',docs))
wait(api('/indexes/movies/settings',{'filterableAttributes':['year','genres'],'sortableAttributes':['year']},'PATCH'))
wait(api('/indexes',{'uid':'empty_index','primaryKey':'book_id'}))
wait(api('/indexes/empty_index/settings',{'embedders':{'manual':{'source':'userProvided','dimensions':3}}},'PATCH'))
print('Seeded 205 movies and an empty custom-PK index')
