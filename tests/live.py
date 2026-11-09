"""Live JSON-RPC regression suite against a dedicated local Meilisearch."""
import json, os, subprocess, unittest, urllib.request, urllib.error
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = os.environ.get('MEILI_TEST_URL', 'http://127.0.0.1:17700')
KEY = os.environ.get('MEILI_TEST_KEY', 'tabularis-meili-local-demo-key')
BINARY = os.environ.get('MEILI_TEST_BINARY', str(ROOT / 'target/release/meilisearch'))

class Live(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.proc = subprocess.Popen([BINARY], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        from urllib.parse import urlparse
        u = urlparse(BASE)
        cls.conn = {'host':u.hostname,'port':u.port or 7700,'password':KEY,'ssl_mode':'disable'}

    @classmethod
    def tearDownClass(cls):
        cls.proc.stdin.close(); cls.proc.wait(timeout=10); cls.proc.stdout.close()

    def call(self, method, **args):
        self.proc.stdin.write(json.dumps({'jsonrpc':'2.0','id':1,'method':method,'params':{'params':self.conn, **args}})+'\n')
        self.proc.stdin.flush()
        return json.loads(self.proc.stdout.readline())

    def result(self, method, **args):
        r=self.call(method,**args); self.assertNotIn('error',r,r); return r['result']

    def document(self, ident):
        req=urllib.request.Request(BASE+'/indexes/movies/documents/'+str(ident),headers={'Authorization':'Bearer '+KEY})
        return json.load(urllib.request.urlopen(req))

    def test_authentication(self):
        self.result('test_connection')
        r=self.call('test_connection',params={**self.conn,'password':'wrong-test-key'})
        self.assertIn('error',r); self.assertIn('invalid_api_key',r['error']['message'])
        self.assertNotIn(KEY,r['error']['message'])

    def test_metadata_and_empty_index(self):
        self.assertEqual(self.result('get_databases'), ['meilisearch'])
        self.assertTrue({'movies','empty_index'} <= {r['name'] for r in self.result('get_tables')})
        cols=self.result('get_columns',table='movies'); self.assertTrue(next(r for r in cols if r['name']=='id')['is_pk'])
        self.assertEqual(self.result('get_columns',table='empty_index')[0]['name'],'book_id')
        self.assertIn('error',self.call('get_columns',table='absent_index'))

    def test_pagination(self):
        a=self.result('execute_query',query='SELECT * FROM "movies"',limit=2,page=1)
        b=self.result('execute_query',query='SELECT * FROM "movies"',limit=2,page=2)
        self.assertEqual(len(a['rows']),2); self.assertEqual(len(b['rows']),2)
        self.assertNotEqual(a['rows'][0],b['rows'][0]); self.assertEqual(b['pagination']['page_size'],2)
        self.assertEqual(b['pagination']['total_rows'],205); self.assertTrue(b['pagination']['has_more'])
        end=self.result('execute_query',query='SELECT * FROM "movies"',limit=2,page=104)
        self.assertEqual(end['rows'],[]); self.assertFalse(end['pagination']['has_more'])
        self.assertIn('error',self.call('execute_query',query='SELECT * FROM "movies"',limit=0))

    def test_native_search_filters_facets(self):
        q='@meili-search:'+json.dumps({'index':'movies','q':'matrx','facets':['genres'],'filter':'year = 1999'})
        r=self.result('execute_query',query=q)
        self.assertEqual(len(r['rows']),1); self.assertIn('The Matrix',str(r['rows']))
        r=self.result('execute_query',query='@meili-search:{"index":"movies","q":"no_such_title_xyz"}')
        self.assertEqual(r['rows'],[])
        r=self.result('execute_query',query='@meili-search:{"index":"movies","q":"","filter":"year = 2024","sort":["year:desc"],"limit":3}')
        self.assertEqual(len(r['rows']),3)

    def test_unsupported_and_invalid_queries(self):
        for q in ['DELETE FROM movies','SELECT * FROM movies WHERE year = 1999','SELECT * FROM movies ORDER BY id','@meili-search:{"index":"../keys"}','@meili-search:{"index":"movies","limit":0}','@meili-search:{']:
            self.assertIn('error',self.call('execute_query',query=q),q)
        self.assertEqual(self.call('get_create_table_sql')['error']['code'],-32601)

    def test_failed_async_write_is_reported(self):
        r=self.call('insert_record',table='empty_index',data={'book_id':'bad_vector','_vectors':{'manual':[1,2]}})
        self.assertIn('error',r)
        self.assertIn('task',r['error']['message'])

    def test_missing_document_update_does_not_upsert(self):
        r=self.call('update_record',table='movies',pk_map={'id':'absent_test'},col_name='title',new_val='No upsert')
        self.assertIn('error',r)

    def test_write_roundtrip_and_protection(self):
        data={'id':'e2e-new','title':'Unicode café 🦊','year':2026,'genres':['Test'],'details':{'nested':True}}
        self.assertEqual(self.result('insert_record',table='movies',data=data),1)
        self.assertEqual(self.document('e2e-new'),data)
        self.assertIn('error',self.call('insert_record',table='movies',data={'id':'e2e-new','title':'Must not replace'}))
        self.assertEqual(self.document('e2e-new'),data)
        self.assertEqual(self.result('update_record',table='movies',pk_map={'id':'e2e-new'},col_name='title',new_val='Updated'),1)
        self.assertEqual(self.document('e2e-new')['details'],{'nested':True})
        self.assertEqual(self.document('e2e-new')['title'],'Updated')
        self.assertIn('error',self.call('update_record',table='movies',pk_map={'id':'e2e-new'},col_name='id',new_val='other'))
        self.assertIn('error',self.call('delete_record',table='movies',pk_map={}))
        self.assertEqual(self.result('delete_record',table='movies',pk_map={'id':'e2e-new'}),1)
        with self.assertRaises(urllib.error.HTTPError): self.document('e2e-new')

if __name__=='__main__': unittest.main(verbosity=2)
